use std::cell::RefCell;
use std::sync::{Arc, Mutex, RwLock};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format};

use comrak::nodes::{AstNode, NodeHeading, NodeLink, NodeValue};
use egui::ahash::HashMap;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{
    Grapheme, Graphemes, IntoRangeExt as _, RangeExt as _, RangeIterExt as _,
};

use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_editor::widget::inline::html_inline::FOLD_TAG;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::{Event, MdRender};

pub(crate) mod container;
pub(crate) mod leaf;
pub(crate) mod spacing;

impl<'ast> MdRender {
    pub fn width(&self, node: &'ast AstNode<'ast>) -> f32 {
        let parent = || node.parent().unwrap();
        let parent_width = || self.width(parent());
        let parent_indent = || self.indent(parent());
        // `parent_width - parent_indent` can go negative for deeply
        // nested containers at narrow doc widths (e.g. a table cell
        // inside three blockquotes at ~200px). Clamp to 0 so child
        // computations don't propagate nonsense; `show_block` /
        // `height` separately bail when width falls below the
        // "can't fit anything" threshold.
        let indented_width = || (parent_width() - parent_indent()).max(0.0);

        let value = &node.data.borrow().value;
        let sp = &node.data.borrow().sourcepos;
        match value {
            NodeValue::FrontMatter(_) => self.width,
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => indented_width(),
            NodeValue::BlockQuote => indented_width(),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.width,
            NodeValue::FootnoteDefinition(_) => indented_width(),
            NodeValue::Item(_) => indented_width(),
            NodeValue::List(_) => indented_width(), // indentation handled by items
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => indented_width(),
            NodeValue::TableRow(_) => indented_width(),
            NodeValue::TaskItem(_) => indented_width(),

            // inline
            NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::Highlight
            | NodeValue::HtmlInline(_)
            | NodeValue::Image(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::ShortCode(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
            | NodeValue::Subtext
            | NodeValue::Superscript
            | NodeValue::Text(_)
            | NodeValue::Underline
            | NodeValue::WikiLink(_) => unimplemented!("not a block: {} {:?}", sp, value),

            // leaf_block
            NodeValue::CodeBlock(_) => indented_width(),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => indented_width(),
            NodeValue::HtmlBlock(_) => indented_width(),
            NodeValue::Paragraph => indented_width(),
            NodeValue::TableCell => self.width_table_cell(node),
            NodeValue::ThematicBreak => indented_width(),
        }
    }

    pub fn height(&self, node: &'ast AstNode<'ast>) -> f32 {
        if let Some(cached) = self.get_cached_node_height(node) {
            return cached;
        }

        // Block too narrow to fit anything meaningful: render nothing.
        // `show_block` short-circuits on the same condition, so this
        // matches what gets painted.
        if self.width(node) < self.layout.row_height {
            self.set_cached_node_height(node, 0.0);
            return 0.0;
        }

        // container blocks: if revealed, show source lines instead
        if node.parent().is_some()
            && node.data.borrow().value.is_container_block()
            && self.reveal(node)
        {
            let mut height = 0.;

            for line in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line];
                let node_line = self.node_line(node, line);

                height += self.height_section(
                    &mut self.new_wrap(self.width(node)),
                    node_line,
                    self.text_format_syntax(),
                );
                height += self.layout.block_spacing;
            }
            if height > 0. {
                height -= self.layout.block_spacing;
            }

            return height;
        }

        // hide folded nodes only if they are not revealed
        if self.hidden_by_fold(node) {
            return 0.;
        }

        let value = &node.data.borrow().value;
        let sp = &node.data.borrow().sourcepos;
        let height = match value {
            NodeValue::FrontMatter(_) => self.height_front_matter(node),
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(node_alert) => self.height_alert(node, node_alert),
            NodeValue::BlockQuote => self.height_block_quote(node),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.height_document(node),
            NodeValue::FootnoteDefinition(_) => self.height_footnote_definition(node),
            NodeValue::Item(_) => self.height_item(node),
            NodeValue::List(_) => self.block_children_height(node),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => self.height_table(node),
            NodeValue::TableRow(_) => self.height_table_row(node),
            NodeValue::TaskItem(_) => self.height_task_item(node),

            // inline
            NodeValue::Image(node_link) => {
                let NodeLink { url, .. } = &**node_link;
                self.height_image(node, url) // used when rendering the image itself
            }
            NodeValue::Subtext
            | NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::Highlight
            | NodeValue::HtmlInline(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::ShortCode(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
            | NodeValue::Superscript
            | NodeValue::Text(_)
            | NodeValue::Underline
            | NodeValue::WikiLink(_) => unimplemented!("not a block: {} {:?}", sp, value),

            // leaf_block
            NodeValue::CodeBlock(node_code_block) => self.height_code_block(node, node_code_block),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(NodeHeading { level, setext, .. }) => {
                self.height_heading(node, *level, *setext)
            }
            NodeValue::HtmlBlock(_) => self.height_html_block(node),
            NodeValue::Paragraph => self.height_paragraph(node),
            NodeValue::TableCell => self.height_table_cell(node),
            NodeValue::ThematicBreak => self.height_thematic_break(),
        };

        self.set_cached_node_height(node, height);

        height
    }

    /// Cheap height estimate keyed on char count × presumed char
    /// width, no cosmic-text shaping. Drifts from precise layout —
    /// safe for off-screen sizing (scrollbar) only; visible content
    /// must use [`Self::height`].
    pub fn height_approx(&self, node: &'ast AstNode<'ast>) -> f32 {
        if let Some(cached) = self.get_cached_node_height_approx(node) {
            return cached;
        }
        if self.hidden_by_fold(node) {
            self.set_cached_node_height_approx(node, 0.);
            return 0.;
        }
        if self.width(node) < self.layout.row_height {
            self.set_cached_node_height_approx(node, 0.);
            return 0.;
        }
        let value = &node.data.borrow().value;
        let height = match value {
            NodeValue::Document
            | NodeValue::List(_)
            | NodeValue::BlockQuote
            | NodeValue::Item(_)
            | NodeValue::TaskItem(_)
            | NodeValue::Alert(_)
            | NodeValue::Table(_)
            | NodeValue::TableRow(_)
            | NodeValue::FootnoteDefinition(_) => {
                let mut total = 0.;
                for child in node.children() {
                    total += self.block_pre_spacing_height_approx(child);
                    total += self.height_approx(child);
                    total += self.block_post_spacing_height_approx(child);
                }
                total
            }
            NodeValue::Paragraph | NodeValue::Heading(_) | NodeValue::TableCell => {
                let row_height = self.row_height(node);
                let width = self.width(node).max(row_height);
                let mut chars = 0usize;
                for d in node.descendants() {
                    match &d.data.borrow().value {
                        NodeValue::Text(t) => chars += t.chars().count(),
                        NodeValue::Code(c) => chars += c.literal.chars().count(),
                        NodeValue::HtmlInline(s) => chars += s.chars().count(),
                        NodeValue::Math(m) => chars += m.literal.chars().count(),
                        NodeValue::SoftBreak | NodeValue::LineBreak => chars += 1,
                        _ => {}
                    }
                }
                let char_width = row_height * 0.5;
                let chars_per_row = (width / char_width).floor().max(1.0) as usize;
                let rows = ((chars as f32) / chars_per_row as f32).ceil().max(1.0);
                rows * row_height + (rows - 1.0).max(0.0) * self.layout.row_spacing
            }
            NodeValue::CodeBlock(_) | NodeValue::HtmlBlock(_) => {
                let row_height = self.row_height(node);
                let n = (self.node_last_line_idx(node) - self.node_first_line_idx(node) + 1) as f32;
                n * row_height + (n - 1.0).max(0.0) * self.layout.row_spacing
            }
            NodeValue::ThematicBreak => self.height_thematic_break(),
            NodeValue::FrontMatter(_) => self.height_front_matter(node),
            _ => 0.,
        };
        self.set_cached_node_height_approx(node, height);
        height
    }

    /// Approx-y of the top of the last top-level block — i.e., the
    /// scroll offset at which the last block's top sits at viewport
    /// top. Function of doc + width only.
    pub fn approx_y_top_last_block(&self, root: &'ast AstNode<'ast>) -> f32 {
        let last = match root.last_child() {
            Some(l) => l,
            None => return 0.,
        };
        let mut y = 0.;
        let mut child = root.first_child();
        while let Some(c) = child {
            if std::ptr::eq(c, last) {
                break;
            }
            y += self.block_pre_spacing_height_approx(c);
            y += self.height_approx(c);
            y += self.block_post_spacing_height_approx(c);
            child = c.next_sibling();
        }
        y += self.block_pre_spacing_height_approx(last);
        y
    }

    /// Approx scroll extent — used to seed the affine widget's
    /// `max_offset` when reading persisted state.
    pub fn scroll_extent(&self, root: &'ast AstNode<'ast>, viewport_height: f32) -> f32 {
        self.approx_y_top_last_block(root) + viewport_height
    }

    pub(crate) fn show_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let ui = &mut self.node_ui(ui, node);

        // Block too narrow to fit anything meaningful: skip. `height`
        // returns 0 for the same condition so the layout stays aligned.
        if self.width(node) < self.layout.row_height {
            return;
        }

        // container blocks: if revealed, show source lines instead
        if node.parent().is_some()
            && node.data.borrow().value.is_container_block()
            && self.reveal(node)
        {
            for line in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line];
                let node_line = self.node_line(node, line);

                let mut wrap = self.new_wrap(self.width(node));
                self.show_section(ui, top_left, &mut wrap, node_line, self.text_format_syntax());

                top_left.y += wrap.height();
                top_left.y += self.layout.block_spacing;
                self.bounds.wrap_lines.extend(wrap.row_ranges);
            }

            return;
        }

        // hide folded nodes only if they are not revealed
        if self.hidden_by_fold(node) {
            return;
        }

        let value = &node.data.borrow().value;
        let sp = &node.data.borrow().sourcepos;
        match value {
            NodeValue::FrontMatter(_) => self.show_front_matter(ui, node, top_left),
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(node_alert) => self.show_alert(ui, node, top_left, node_alert),
            NodeValue::BlockQuote => self.show_block_quote(ui, node, top_left),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.show_document(ui, node, top_left),
            NodeValue::FootnoteDefinition(_) => self.show_footnote_definition(ui, node, top_left),
            NodeValue::Item(_) => self.show_item(ui, node, top_left),
            NodeValue::List(_) => self.show_block_children(ui, node, top_left),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => self.show_table(ui, node, top_left),
            NodeValue::TableRow(is_header_row) => {
                self.show_table_row(ui, node, top_left, *is_header_row)
            }
            NodeValue::TaskItem(node_task_item) => {
                self.show_task_item(ui, node, top_left, node_task_item)
            }

            // inline
            NodeValue::Image(node_link) => {
                let NodeLink { url, .. } = &**node_link;
                self.show_image_block(ui, node, top_left, url)
            }
            NodeValue::Subtext
            | NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::Highlight
            | NodeValue::HtmlInline(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::ShortCode(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
            | NodeValue::Superscript
            | NodeValue::Text(_)
            | NodeValue::Underline
            | NodeValue::WikiLink(_) => unimplemented!("not a block: {} {:?}", sp, value),

            // leaf_block
            NodeValue::CodeBlock(node_code_block) => {
                self.show_code_block(ui, node, top_left, node_code_block)
            }
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(NodeHeading { level, setext, .. }) => {
                self.show_heading(ui, node, top_left, *level, *setext)
            }
            NodeValue::HtmlBlock(_) => self.show_html_block(ui, node, top_left),
            NodeValue::Paragraph => self.show_paragraph(ui, node, top_left),
            NodeValue::TableCell => self.show_table_cell(ui, node, top_left),
            NodeValue::ThematicBreak => self.show_thematic_break(ui, node, top_left),
        }
    }

    /// Returns true if the given block node is selected for the purposes of
    /// rich editing. All selected nodes are siblings at any given time.
    pub fn selected_block(&self, node: &'ast AstNode<'ast>) -> bool {
        // the document is never selected
        let Some(parent) = node.parent() else {
            return false;
        };

        self.node_intersects_selection(node)
            && self.node_contains_selection(parent)
            && (node.is_container_block() && !self.node_contains_selection(node)
                || node.is_leaf_block())
    }

    /// Returns the portion of the line that's within the node, excluding line
    /// prefixes due to parent nodes. For container blocks, this is equivalent
    /// to [`line_own_prefix`] + [`line_content`]. For leaf blocks, which have
    /// no prefix, this is equivalent to [`line_content`].
    pub fn node_line(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> (Grapheme, Grapheme) {
        let Some(parent) = node.parent() else { return line }; // document has no prefix
        let (parent_prefix_len, _) = self.line_prefix_len(parent, line);

        (line.start() + parent_prefix_len, line.end())
    }

    /// Returns the (inclusive, exclusive) range of lines that this node is sourced from.
    pub fn node_lines(&self, node: &'ast AstNode<'ast>) -> (usize, usize) {
        self.range_lines(self.node_range(node))
    }

    /// Returns the line index of the first line of a node
    pub fn node_first_line_idx(&self, node: &'ast AstNode<'ast>) -> usize {
        self.bounds
            .source_lines
            .find_containing(self.node_range(node).start(), true, true)
            .start()
    }

    /// Returns the line index of the last line of a node
    pub fn node_last_line_idx(&self, node: &'ast AstNode<'ast>) -> usize {
        self.bounds
            .source_lines
            .find_containing(self.node_range(node).end(), true, true)
            .start()
    }

    /// Returns the first line, the whole first line, and nothing but the first
    /// line of the given node.
    pub fn node_first_line(&self, node: &'ast AstNode<'ast>) -> (Grapheme, Grapheme) {
        self.bounds.source_lines[self.node_first_line_idx(node)]
    }

    /// Returns the last line, the whole last line, and nothing but the last line
    /// of the given node.
    pub fn node_last_line(&self, node: &'ast AstNode<'ast>) -> (Grapheme, Grapheme) {
        self.bounds.source_lines[self.node_last_line_idx(node)]
    }

    /// Returns the node that should have a fold node appended to make this node folded, if there is one
    pub fn foldable(&self, node: &'ast AstNode<'ast>) -> Option<&'ast AstNode<'ast>> {
        // must not be the first block in a list or in a block quote, since then
        // we have no space to render the fold button (matches Bear)
        if let Some(parent) = node.parent() {
            if matches!(parent.data.borrow().value, NodeValue::List(_)) {
                if let Some(grandparent) = parent.parent() {
                    if matches!(grandparent.data.borrow().value, NodeValue::Item(_))
                        && self.node_first_line_idx(node) == self.node_first_line_idx(grandparent)
                    {
                        return None;
                    }
                    if matches!(grandparent.data.borrow().value, NodeValue::BlockQuote) {
                        return None;
                    }
                }
            } else {
                if matches!(parent.data.borrow().value, NodeValue::Item(_))
                    && self.node_first_line_idx(node) == self.node_first_line_idx(parent)
                {
                    return None;
                }
                if matches!(parent.data.borrow().value, NodeValue::BlockQuote) {
                    return None;
                }
            }
        }

        // list items
        if matches!(node.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_)) {
            // must have paragraph to add fold html tag + something to fold
            if node.children().count() < 2 {
                return None;
            }

            if let Some(first_child_block) = node.first_child() {
                if first_child_block.data.borrow().value == NodeValue::Paragraph {
                    return Some(first_child_block);
                }
            }
        }

        // headings
        if let NodeValue::Heading(heading) = node.data.borrow().value {
            // must have something to fold
            if let Some(next_sibling) = node.next_sibling() {
                if let NodeValue::Heading(next_heading) = next_sibling.data.borrow().value {
                    if next_heading.level <= heading.level {
                        return None;
                    }
                }
            } else {
                return None;
            }

            return Some(node);
        }

        None
    }

    /// Returns the fold node - the node that is causing this node to be folded - if there is one
    pub fn fold(&self, node: &'ast AstNode<'ast>) -> Option<&'ast AstNode<'ast>> {
        if let Some(foldable) = self.foldable(node) {
            for inline in foldable.children() {
                if let NodeValue::HtmlInline(html) = &inline.data.borrow().value {
                    if html == FOLD_TAG {
                        return Some(inline);
                    }
                }
            }
        }
        None
    }

    #[allow(clippy::collapsible_else_if)]
    pub fn apply_fold(
        &mut self, node: &'ast AstNode<'ast>, contents: (Grapheme, Grapheme), unapply: bool,
    ) {
        if unapply {
            if let Some(fold) = self.fold(node) {
                self.render_events.push(Event::Replace {
                    region: self.node_range(fold).into(),
                    text: "".into(),
                    advance_cursor: false,
                });
            }
        } else {
            if let Some(foldable) = self.foldable(node) {
                self.render_events.push(Event::Replace {
                    region: self.node_range(foldable).end().into_range().into(),
                    text: FOLD_TAG.into(),
                    advance_cursor: false,
                });

                // when folding a section that intersects the cursor, adjust the selection
                // this ensures the folded section appears folded / avoids immediate selection reveal
                let selection = self.buffer.current.selection;

                if contents.intersects(&selection, true)
                    && !selection.contains_range(&contents, true, true)
                {
                    self.render_events.push(Event::Select {
                        region: (
                            selection.start().min(contents.start()),
                            selection.end().min(contents.start()),
                        )
                            .into(),
                    });
                }
            }
        }
    }

    /// Returns the node that this node is folding, if there is one
    pub fn foldee(&self, node: &'ast AstNode<'ast>) -> Option<&'ast AstNode<'ast>> {
        let mut root = node;
        while let Some(parent) = root.parent() {
            root = parent;
        }

        for descendant in root.descendants() {
            if let Some(fold) = self.fold(descendant) {
                if fold.same_node(node) {
                    return Some(fold);
                }
            }
        }

        None
    }

    pub fn hidden_by_fold(&self, node: &'ast AstNode<'ast>) -> bool {
        self.get_cached_hidden_by_fold(node)
            .expect("hidden_by_fold queried for a node not in the current AST")
    }

    /// One DFS over the tree. Carries `item_fold_active` down through
    /// the recursion (any folded-unrevealed Item/TaskItem ancestor
    /// turns it on, hiding all but the first child). Per parent,
    /// maintains a stack of currently-open headings as we walk
    /// children left-to-right; each entry is `(level, folded_unrevealed)`.
    pub fn populate_hidden_by_fold(&self, root: &'ast AstNode<'ast>) {
        // The root is never hidden; cache it directly so a query for
        // `hidden_by_fold(root)` doesn't fall back to the unwrap
        // default.
        self.set_cached_hidden_by_fold(root, false);
        self.populate_hidden_by_fold_subtree(root, false);
    }

    fn populate_hidden_by_fold_subtree(&self, parent: &'ast AstNode<'ast>, item_fold_active: bool) {
        // Per-parent state. Stack invariant: levels strictly
        // increasing from bottom to top.
        let mut heading_stack: Vec<(u8, bool)> = Vec::new();
        let mut sibling_index: usize = 0;
        let mut child = parent.first_child();
        while let Some(c) = child {
            // Pull only the bits we need out of the borrow — copying
            // the heading level and the variant kind avoids cloning
            // the whole `NodeValue` (which carries owned `String`s
            // for inline payloads).
            let (heading_level, is_item_or_task) = {
                let value = &c.data.borrow().value;
                (
                    if let NodeValue::Heading(h) = value { Some(h.level) } else { None },
                    matches!(value, NodeValue::Item(_) | NodeValue::TaskItem(_)),
                )
            };

            // Headings end prior headings at >= their level — pop
            // before the visibility check so the popped headings
            // don't count as containers of this heading.
            if let Some(level) = heading_level {
                while heading_stack.last().is_some_and(|&(lvl, _)| lvl >= level) {
                    heading_stack.pop();
                }
            }

            let hidden_by_item = sibling_index > 0 && item_fold_active;
            // Any folded heading currently on the stack hides us.
            let hidden_by_heading = heading_stack.iter().any(|&(_, folded)| folded);
            self.set_cached_hidden_by_fold(c, hidden_by_item || hidden_by_heading);

            // Push self onto the heading stack so subsequent siblings
            // know we exist as a section container. Even unfolded
            // headings get pushed — they "block" prior less-significant
            // folded headings from contributing past us.
            if let Some(level) = heading_level {
                let folded = self.fold(c).is_some() && !self.heading_fold_reveal(c);
                heading_stack.push((level, folded));
            }

            let child_item_fold_active = item_fold_active
                || (is_item_or_task && self.fold(c).is_some() && !self.item_fold_reveal(c));
            self.populate_hidden_by_fold_subtree(c, child_item_fold_active);

            child = c.next_sibling();
            sibling_index += 1;
        }
    }
}

// Fast integer mapping for NodeValue variants - no hashing needed
fn node_value_to_discriminant_id(value: &NodeValue) -> u8 {
    match value {
        NodeValue::FrontMatter(_) => 1,
        NodeValue::Raw(_) => 2,
        NodeValue::Alert(_) => 3,
        NodeValue::BlockQuote => 4,
        NodeValue::DescriptionItem(_) => 5,
        NodeValue::DescriptionList => 6,
        NodeValue::Document => 7,
        NodeValue::FootnoteDefinition(_) => 8,
        NodeValue::Item(_) => 9,
        NodeValue::List(_) => 10,
        NodeValue::MultilineBlockQuote(_) => 11,
        NodeValue::Table(_) => 12,
        NodeValue::TableRow(_) => 13,
        NodeValue::TaskItem(_) => 14,
        NodeValue::Image(_) => 15,
        NodeValue::Code(_) => 16,
        NodeValue::Emph => 17,
        NodeValue::Escaped => 18,
        NodeValue::EscapedTag(_) => 19,
        NodeValue::FootnoteReference(_) => 20,
        NodeValue::HtmlInline(_) => 21,
        NodeValue::LineBreak => 22,
        NodeValue::Link(_) => 23,
        NodeValue::Math(_) => 24,
        NodeValue::SoftBreak => 25,
        NodeValue::SpoileredText => 26,
        NodeValue::Strikethrough => 27,
        NodeValue::Strong => 28,
        NodeValue::Subscript => 29,
        NodeValue::Superscript => 30,
        NodeValue::Text(_) => 31,
        NodeValue::Underline => 32,
        NodeValue::WikiLink(_) => 33,
        NodeValue::CodeBlock(_) => 34,
        NodeValue::DescriptionDetails => 35,
        NodeValue::DescriptionTerm => 36,
        NodeValue::Heading(_) => 37,
        NodeValue::HtmlBlock(_) => 38,
        NodeValue::Paragraph => 39,
        NodeValue::TableCell => 40,
        NodeValue::ThematicBreak => 41,
        NodeValue::Highlight => 42,
        NodeValue::ShortCode(_) => 43,
        NodeValue::Subtext => 44,
    }
}

pub enum TitleState {
    Loading,
    Loaded(String),
    Failed,
}

type LinePrefixKey = (u64, (Grapheme, Grapheme));
type LinePrefixValue = (Graphemes, bool);

#[derive(Default)]
pub struct LayoutCache {
    pub height: RefCell<HashMap<(Grapheme, Grapheme), f32>>,
    pub height_approx: RefCell<HashMap<(Grapheme, Grapheme), f32>>,
    pub line_prefix_len: RefCell<HashMap<LinePrefixKey, LinePrefixValue>>,
    pub node_range: RefCell<HashMap<u64, (Grapheme, Grapheme)>>,
    pub hidden_by_fold: RefCell<HashMap<(Grapheme, Grapheme), bool>>,
    pub link_titles: RefCell<HashMap<String, Arc<Mutex<TitleState>>>>,
}

impl LayoutCache {
    /// Full clear for width/resize changes where everything must be recomputed.
    pub fn clear(&self) {
        self.height.borrow_mut().clear();
        self.height_approx.borrow_mut().clear();
        self.line_prefix_len.borrow_mut().clear();
        self.node_range.borrow_mut().clear();
        self.hidden_by_fold.borrow_mut().clear();
        // link_titles intentionally not cleared: fetched titles persist across layout invalidations
    }

    /// Invalidation for text changes. Height and hidden_by_fold depend on
    /// fold state (fold tags in text) and on each other (height returns 0
    /// for hidden nodes), so both must be fully cleared — a fold tag
    /// insertion at one point changes heights of distant sibling nodes.
    /// Sourcepos-keyed caches are cleared because the AST is re-parsed.
    /// Glyphon buffers are content-addressed and survive.
    pub fn invalidate_text_change(&self) {
        self.height.borrow_mut().clear();
        self.height_approx.borrow_mut().clear();
        self.hidden_by_fold.borrow_mut().clear();
        self.line_prefix_len.borrow_mut().clear();
        self.node_range.borrow_mut().clear();

        // glyphon_buffers: content-addressed, preserved across text changes
    }

    /// Invalidates height entries affected by a reveal range change (cursor
    /// movement or find match change). A node's height depends on its reveal
    /// state, so we evict nodes intersecting either range plus their ancestors.
    pub fn invalidate_reveal_change(
        &self, old_range: (Grapheme, Grapheme), new_range: (Grapheme, Grapheme),
    ) {
        let mut cache = self.height.borrow_mut();

        // first pass: find ranges directly affected
        let mut invalidated: Vec<(Grapheme, Grapheme)> = Vec::new();
        for range in cache.keys() {
            if range.intersects(&old_range, true) || range.intersects(&new_range, true) {
                invalidated.push(*range);
            }
        }

        if invalidated.is_empty() {
            return;
        }

        // second pass: evict directly affected nodes and their ancestors
        cache.retain(|range, _| {
            if range.intersects(&old_range, true) || range.intersects(&new_range, true) {
                return false;
            }
            for inv in &invalidated {
                if range.contains_range(inv, true, true) {
                    return false;
                }
            }
            true
        });

        // hidden_by_fold depends on selection through fold reveal;
        // a node's visibility can change due to a distant heading/item
        // becoming revealed, so clear the whole cache
        self.hidden_by_fold.borrow_mut().clear();
    }
}

impl MdRender {
    /// Look up or shape a glyphon buffer for the given text and formatting.
    /// Delegates to the shared GlyphonCache so the same shaped buffer is reused
    /// across frames (and across widgets). The editor-specific concern here is
    /// emoji: graphemes containing emoji codepoints get the Twemoji font family
    /// while everything else uses the format's font family.
    /// Default wrap mode: word boundaries preferred, glyph boundaries as a
    /// fallback for over-wide single tokens. This is what `split_rows` reaches
    /// for first when discovering row breaks.
    pub fn upsert_glyphon_buffer(
        &self, text: &str, font_size: f32, line_height: f32, width: f32, format: &Format,
    ) -> Arc<RwLock<glyphon::Buffer>> {
        self.upsert_glyphon_buffer_inner(
            text,
            font_size,
            line_height,
            width,
            format,
            glyphon::Wrap::WordOrGlyph,
        )
    }

    /// Glyph-level wrap. Used as a fallback by `split_rows` when
    /// `WordOrGlyph` produces a layout run wider than the wrap width — a
    /// known cosmic-text quirk on some bold mixed-script content. Glyph
    /// mode breaks at any glyph boundary, which means mid-word splits, but
    /// that's better than overflowing a cell.
    pub fn upsert_glyphon_buffer_glyph(
        &self, text: &str, font_size: f32, line_height: f32, width: f32, format: &Format,
    ) -> Arc<RwLock<glyphon::Buffer>> {
        self.upsert_glyphon_buffer_inner(
            text,
            font_size,
            line_height,
            width,
            format,
            glyphon::Wrap::Glyph,
        )
    }

    /// Like [`Self::upsert_glyphon_buffer`] but with `Wrap::None`. Use for
    /// already-split text when you want a stable single-row shape; the
    /// wrapped variant's break-point decisions vary subtly with input
    /// context (a known cosmic-text quirk), so a piece chosen by one call
    /// may re-wrap differently on a second call at the same width.
    pub fn upsert_glyphon_buffer_unwrapped(
        &self, text: &str, font_size: f32, line_height: f32, width: f32, format: &Format,
    ) -> Arc<RwLock<glyphon::Buffer>> {
        self.upsert_glyphon_buffer_inner(
            text,
            font_size,
            line_height,
            width,
            format,
            glyphon::Wrap::None,
        )
    }

    fn upsert_glyphon_buffer_inner(
        &self, text: &str, font_size: f32, line_height: f32, width: f32, format: &Format,
        wrap_mode: glyphon::Wrap,
    ) -> Arc<RwLock<glyphon::Buffer>> {
        let font_system = self
            .ctx
            .data(|d| d.get_temp::<Arc<Mutex<glyphon::FontSystem>>>(egui::Id::NULL))
            .unwrap();
        let glyphon_cache = self
            .ctx
            .data(|d| {
                d.get_temp::<Arc<Mutex<crate::widgets::glyphon_cache::GlyphonCache>>>(
                    egui::Id::NULL,
                )
            })
            .unwrap();

        let ppi = self.ctx.pixels_per_point();
        let font_size = font_size * ppi;
        let line_height = line_height * ppi;
        let width = width * ppi;

        use crate::widgets::glyphon_cache::*;
        // Fold wrap mode into the cache key via low-bit nudges to width.
        // Three modes need three distinct keys — a sub-ULP perturbation of
        // an already-quantized pixel value, harmless to layout but enough
        // to separate cache entries.
        let width_bits = match wrap_mode {
            glyphon::Wrap::WordOrGlyph => width.to_bits(),
            glyphon::Wrap::None => width.to_bits() ^ 1,
            glyphon::Wrap::Glyph => width.to_bits() ^ 2,
            _ => width.to_bits() ^ 3,
        };
        let key = GlyphonCacheKey::single(
            text,
            match format.family {
                FontFamily::Sans => GlyphonFontFamily::SansSerif,
                FontFamily::Mono => GlyphonFontFamily::Monospace,
                FontFamily::Icons => GlyphonFontFamily::Named("Nerd Fonts Mono Symbols".into()),
            },
            format.bold,
            format.italic,
            Some(format.color.to_array()),
            font_size.to_bits(),
            line_height.to_bits(),
            width_bits,
        );

        let fs = Arc::clone(&font_system);
        let mut cache = glyphon_cache.lock().unwrap();
        cache.get_or_shape(key, move || {
            let attrs = glyphon::Attrs::new()
                .family(match format.family {
                    FontFamily::Sans => glyphon::Family::SansSerif,
                    FontFamily::Mono => glyphon::Family::Monospace,
                    FontFamily::Icons => glyphon::Family::Name("Nerd Fonts Mono Symbols"),
                })
                .weight(if format.bold { glyphon::Weight::BOLD } else { glyphon::Weight::NORMAL })
                .style(if format.italic { glyphon::Style::Italic } else { glyphon::Style::Normal });
            let metrics = glyphon::Metrics::new(font_size, line_height);
            let mut b = glyphon::Buffer::new(&mut fs.lock().unwrap(), metrics);
            b.set_size(&mut fs.lock().unwrap(), Some(width), None);
            let emoji_attrs =
                glyphon::Attrs::new().family(glyphon::Family::Name("Twemoji Mozilla"));
            let text = text.to_string();
            let spans = text.graphemes(true).map(|g| {
                let is_emoji = g.chars().any(|c| {
                    matches!(
                        c as u32,
                        0xFE0F  // variation selector-16: emoji presentation
                    | 0x1F000.. // supplementary multilingual plane: core emoji blocks
                    )
                });
                (g, if is_emoji { emoji_attrs.clone() } else { attrs.clone() })
            });
            b.set_rich_text(
                &mut fs.lock().unwrap(),
                spans,
                &attrs,
                glyphon::Shaping::Advanced,
                None,
            );
            b.set_wrap(&mut fs.lock().unwrap(), wrap_mode);
            b
        })
    }
}

impl<'ast> MdRender {
    pub fn get_cached_node_height(&self, node: &'ast AstNode<'ast>) -> Option<f32> {
        let range = self.node_range(node);
        self.layout_cache.height.borrow().get(&range).copied()
    }

    pub fn set_cached_node_height(&self, node: &'ast AstNode<'ast>, height: f32) {
        let range = self.node_range(node);
        self.layout_cache.height.borrow_mut().insert(range, height);
    }

    pub fn get_cached_node_height_approx(&self, node: &'ast AstNode<'ast>) -> Option<f32> {
        let range = self.node_range(node);
        self.layout_cache
            .height_approx
            .borrow()
            .get(&range)
            .copied()
    }

    pub fn set_cached_node_height_approx(&self, node: &'ast AstNode<'ast>, height: f32) {
        let range = self.node_range(node);
        self.layout_cache
            .height_approx
            .borrow_mut()
            .insert(range, height);
    }

    pub fn get_cached_line_prefix_len(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> Option<(Graphemes, bool)> {
        let node_key_hash = Self::pack_node_key(node);
        self.layout_cache
            .line_prefix_len
            .borrow()
            .get(&(node_key_hash, line))
            .copied()
    }

    pub fn set_cached_line_prefix_len(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme), value: (Graphemes, bool),
    ) {
        let node_key_hash = Self::pack_node_key(node);
        self.layout_cache
            .line_prefix_len
            .borrow_mut()
            .insert((node_key_hash, line), value);
    }

    #[inline]
    pub fn get_cached_node_range(&self, node: &'ast AstNode<'ast>) -> Option<(Grapheme, Grapheme)> {
        let key_hash = Self::pack_node_key(node);
        self.layout_cache
            .node_range
            .borrow()
            .get(&key_hash)
            .copied()
    }

    #[inline]
    pub fn set_cached_node_range(&self, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme)) {
        let key_hash = Self::pack_node_key(node);
        self.layout_cache
            .node_range
            .borrow_mut()
            .insert(key_hash, range);
    }

    pub fn get_cached_hidden_by_fold(&self, node: &'ast AstNode<'ast>) -> Option<bool> {
        let range = self.node_range(node);
        self.layout_cache
            .hidden_by_fold
            .borrow()
            .get(&range)
            .copied()
    }

    pub fn set_cached_hidden_by_fold(&self, node: &'ast AstNode<'ast>, hidden: bool) {
        let range = self.node_range(node);
        self.layout_cache
            .hidden_by_fold
            .borrow_mut()
            .insert(range, hidden);
    }

    /// Pack node info into u64 using bit manipulation - ultra fast cache key
    fn pack_node_key(node: &AstNode) -> u64 {
        let borrowed = node.data.borrow();
        let sp = borrowed.sourcepos;
        let (start_line, start_column, end_line, end_column, discriminant) = (
            sp.start.line as u64,
            sp.start.column as u64,
            sp.end.line as u64,
            sp.end.column as u64,
            node_value_to_discriminant_id(&borrowed.value) as u64,
        );

        // Pack into 64 bits: 15 bits each for start_line, start_column, end_line, end_column
        // and 4 bits for discriminant (total: 15+15+15+15+4 = 64 bits exactly)
        // Use bitwise AND for fastest truncation
        ((start_line & 0x7FFF) << 49)
            | ((start_column & 0x7FFF) << 34)
            | ((end_line & 0x7FFF) << 19)
            | ((end_column & 0x7FFF) << 4)
            | (discriminant & 0xF)
    }
}
