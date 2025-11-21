use std::cell::RefCell;

use comrak::nodes::{AstNode, NodeHeading, NodeLink, NodeValue};
use egui::ahash::HashMap;
use egui::{Id, Pos2, Rect, Ui};
use lb_rs::model::text::offset_types::{
    DocCharOffset, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_editor::widget::BLOCK_SPACING;
use crate::tab::markdown_editor::widget::inline::html_inline::FOLD_TAG;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

pub(crate) mod container;
pub(crate) mod leaf;
pub(crate) mod spacing;

impl<'ast> Editor {
    pub fn width(&self, node: &'ast AstNode<'ast>) -> f32 {
        let parent = || node.parent().unwrap();
        let parent_width = || self.width(parent());
        let parent_indent = || self.indent(parent());
        let indented_width = || parent_width() - parent_indent();

        let value = &node.data.borrow().value;
        let sp = &node.data.borrow().sourcepos;
        match value {
            NodeValue::FrontMatter(_) => 0.,
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
            NodeValue::Image(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Code(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Emph => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Escaped => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::EscapedTag(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::FootnoteReference(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::HtmlInline(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::LineBreak => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Link(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Math(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::SoftBreak => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::SpoileredText => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Strikethrough => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Strong => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Subscript => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Superscript => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Text(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Underline => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::WikiLink(_) => unimplemented!("not a block: {} {:?}", sp, value),

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
                    &mut Wrap::new(self.width(node)),
                    node_line,
                    self.text_format_syntax(node),
                );
                height += BLOCK_SPACING;
            }
            if height > 0. {
                height -= BLOCK_SPACING;
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
            NodeValue::FrontMatter(_) => 0.,
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(node_alert) => self.height_alert(node, node_alert),
            NodeValue::BlockQuote => self.height_block_quote(node),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.block_children_height(node),
            NodeValue::FootnoteDefinition(_) => self.height_footnote_definition(node),
            NodeValue::Item(_) => self.height_item(node),
            NodeValue::List(_) => self.block_children_height(node),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => self.height_table(node),
            NodeValue::TableRow(_) => self.height_table_row(node),
            NodeValue::TaskItem(_) => self.height_task_item(node),

            // inline
            NodeValue::Image(NodeLink { url, .. }) => self.height_image(node, url), // used when rendering the image itself
            NodeValue::Code(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Emph => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Escaped => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::EscapedTag(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::FootnoteReference(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::HtmlInline(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::LineBreak => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Link(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Math(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::SoftBreak => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::SpoileredText => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Strikethrough => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Strong => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Subscript => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Superscript => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Text(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Underline => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::WikiLink(_) => unimplemented!("not a block: {} {:?}", sp, value),

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

    pub(crate) fn show_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        // container blocks: if revealed, show source lines instead
        if node.parent().is_some()
            && node.data.borrow().value.is_container_block()
            && self.reveal(node)
        {
            for line in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line];
                let node_line = self.node_line(node, line);

                let mut wrap = Wrap::new(self.width(node));
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    node_line,
                    self.text_format_syntax(node),
                    false,
                );

                top_left.y += wrap.height();
                top_left.y += BLOCK_SPACING;
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
            NodeValue::FrontMatter(_) => {}
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
            NodeValue::TaskItem(maybe_check) => {
                self.show_task_item(ui, node, top_left, *maybe_check)
            }

            // inline
            NodeValue::Image(NodeLink { url, .. }) => {
                self.show_image_block(ui, node, top_left, url)
            }
            NodeValue::Code(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Emph => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Escaped => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::EscapedTag(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::FootnoteReference(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::HtmlInline(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::LineBreak => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Link(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Math(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::SoftBreak => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::SpoileredText => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Strikethrough => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Strong => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Subscript => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Superscript => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Text(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Underline => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::WikiLink(_) => unimplemented!("not a block: {} {:?}", sp, value),

            // leaf_block
            NodeValue::CodeBlock(node_code_block) => {
                self.show_code_block(ui, node, top_left, node_code_block)
            }
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(NodeHeading { level, setext }) => {
                self.show_heading(ui, node, top_left, *level, *setext)
            }
            NodeValue::HtmlBlock(_) => self.show_html_block(ui, node, top_left),
            NodeValue::Paragraph => self.show_paragraph(ui, node, top_left),
            NodeValue::TableCell => self.show_table_cell(ui, node, top_left),
            NodeValue::ThematicBreak => self.show_thematic_break(ui, node, top_left),
        }
    }

    /// Creates a UI at the given position that assigns ids via the node range.
    // By default, egui ids are assigned to ui's and widgets based on the parent
    // ui's id and incremented with each addition to a given parent. Because
    // editor text may be clickable, text allocates ids and affects future ids.
    // When the editor reveal state changes, more or fewer interactable text
    // units may be shown, and all assigned ids may change. When an iOS user
    // taps the editor, iOS first sends a selection event in a standalone frame
    // which affects the reveal state, then by the time the tap is released, the
    // widget being tapped may have had its id changed and will not register as
    // clicked. This function creates a consistently idenified ui based on the
    // node range to prevent ids from changing mid tap and therefore prevents
    // taps from failing. Note that this range does not and need not survive
    // edits to the document itself.
    fn node_ui(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) -> Ui {
        Ui::new(
            ui.ctx().clone(),
            ui.layer_id(),
            Id::new(self.node_range(node)), // <- the magic
            Rect::from_pos(top_left),
            ui.painter().clip_rect(),
            Default::default(),
        )
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

    /// Returns the children of the given node in sourcepos order.
    pub fn sorted_children(&self, node: &'ast AstNode<'ast>) -> Vec<&'ast AstNode<'ast>> {
        let mut children = Vec::with_capacity(node.children().count());
        children.extend(node.children());
        children.sort_by_key(|c| c.data.borrow().sourcepos);
        children
    }

    /// Returns the siblings of the given node in sourcepos order (unlike
    /// `node.siblings()`).
    pub fn sorted_siblings(&self, node: &'ast AstNode<'ast>) -> Vec<&'ast AstNode<'ast>> {
        if let Some(parent) = node.parent() { self.sorted_children(parent) } else { vec![node] }
    }

    pub fn sibling_index(
        &self, node: &'ast AstNode<'ast>, sorted_siblings: &[&'ast AstNode<'ast>],
    ) -> usize {
        let this_sibling_index = sorted_siblings
            .iter()
            .position(|sibling| node.same_node(sibling))
            .unwrap();

        this_sibling_index
    }

    /// Returns the portion of the line that's within the node, excluding line
    /// prefixes due to parent nodes. For container blocks, this is equivalent
    /// to [`line_own_prefix`] + [`line_content`]. For leaf blocks, which have
    /// no prefix, this is equivalent to [`line_content`].
    pub fn node_line(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> (DocCharOffset, DocCharOffset) {
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
    pub fn node_first_line(&self, node: &'ast AstNode<'ast>) -> (DocCharOffset, DocCharOffset) {
        self.bounds.source_lines[self.node_first_line_idx(node)]
    }

    /// Returns the last line, the whole last line, and nothing but the last line
    /// of the given node.
    pub fn node_last_line(&self, node: &'ast AstNode<'ast>) -> (DocCharOffset, DocCharOffset) {
        self.bounds.source_lines[self.node_last_line_idx(node)]
    }

    /// Returns the node that should have a fold node appended to make this node folded, if there is one
    pub fn foldable(&self, node: &'ast AstNode<'ast>) -> Option<&'ast AstNode<'ast>> {
        // must not be the first block in a list or in a block quote, since then
        // we have no space to render the fold button (matches Bear)
        if let Some(parent) = node.parent() {
            if matches!(parent.data.borrow().value, NodeValue::List(_)) {
                if let Some(grandparent) = parent.parent() {
                    if matches!(grandparent.data.borrow().value, NodeValue::Item(_)) {
                        return None;
                    }
                    if matches!(grandparent.data.borrow().value, NodeValue::BlockQuote) {
                        return None;
                    }
                }
            } else {
                if matches!(parent.data.borrow().value, NodeValue::Item(_)) {
                    return None;
                }
                if matches!(parent.data.borrow().value, NodeValue::BlockQuote) {
                    return None;
                }
            }
        }

        // list items
        if matches!(node.data.borrow().value, NodeValue::Item(_)) {
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
        // show only the first block in folded ancestor blocks
        if node.previous_sibling().is_some() {
            for ancestor in node.ancestors().skip(1) {
                if self.fold(ancestor).is_some() {
                    return true;
                }
            }
        }

        // show only the blocks that have no folded heading; headings with
        // another equal or more significant heading between them and the target
        // node don't count
        let sorted_siblings = self.sorted_siblings(node);
        let sibling_index = self.sibling_index(node, &sorted_siblings);
        let mut most_significant_unfolded_heading =
            if let NodeValue::Heading(heading) = &node.data.borrow().value {
                heading.level
            } else {
                7 // max heading level + 1
            };

        for sibling in sorted_siblings[0..sibling_index].iter().rev() {
            if let NodeValue::Heading(heading) = &sibling.data.borrow().value {
                if heading.level < most_significant_unfolded_heading {
                    if self.fold(sibling).is_some() {
                        return true;
                    }
                    most_significant_unfolded_heading = heading.level;
                }
            }
        }

        false
    }
}

#[derive(Default)]
pub struct CacheEntry<T> {
    range: (DocCharOffset, DocCharOffset),
    value: T,
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
    }
}

pub struct LinePrefixCacheEntry {
    node_key_hash: u64,
    line: (DocCharOffset, DocCharOffset),
    value: (RelCharOffset, bool),
}

#[derive(Default)]
pub struct LayoutCache {
    pub height: RefCell<Vec<CacheEntry<f32>>>,
    pub line_prefix_len: RefCell<Vec<LinePrefixCacheEntry>>,
    pub node_range: RefCell<HashMap<u64, (DocCharOffset, DocCharOffset)>>,
}

impl LayoutCache {
    pub fn clear(&self) {
        self.height.borrow_mut().clear();
        self.line_prefix_len.borrow_mut().clear();
        self.node_range.borrow_mut().clear();
    }
}

impl<'ast> Editor {
    pub fn get_cached_node_height(&self, node: &'ast AstNode<'ast>) -> Option<f32> {
        let range = self.node_range(node);
        self.layout_cache
            .height
            .borrow()
            .binary_search_by(|entry| entry.range.cmp(&range))
            .ok()
            .map(|i| self.layout_cache.height.borrow()[i].value)
    }

    pub fn set_cached_node_height(&self, node: &'ast AstNode<'ast>, height: f32) {
        let range = self.node_range(node);
        let mut cache = self.layout_cache.height.borrow_mut();
        match cache.binary_search_by(|entry| entry.range.cmp(&range)) {
            Ok(i) => cache[i].value = height,
            Err(i) => cache.insert(i, CacheEntry { range, value: height }),
        }
    }

    pub fn get_cached_line_prefix_len(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> Option<(RelCharOffset, bool)> {
        let node_key_hash = Self::pack_node_key(node);
        self.layout_cache
            .line_prefix_len
            .borrow()
            .binary_search_by(|entry| {
                entry
                    .node_key_hash
                    .cmp(&node_key_hash)
                    .then(entry.line.cmp(&line))
            })
            .ok()
            .map(|i| self.layout_cache.line_prefix_len.borrow()[i].value)
    }

    pub fn set_cached_line_prefix_len(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        value: (RelCharOffset, bool),
    ) {
        let node_key_hash = Self::pack_node_key(node);
        let mut cache = self.layout_cache.line_prefix_len.borrow_mut();
        match cache.binary_search_by(|entry| {
            entry
                .node_key_hash
                .cmp(&node_key_hash)
                .then(entry.line.cmp(&line))
        }) {
            Ok(i) => cache[i].value = value,
            Err(i) => cache.insert(i, LinePrefixCacheEntry { node_key_hash, line, value }),
        }
    }

    #[inline]
    pub fn get_cached_node_range(
        &self, node: &'ast AstNode<'ast>,
    ) -> Option<(DocCharOffset, DocCharOffset)> {
        let key_hash = Self::pack_node_key(node);
        self.layout_cache
            .node_range
            .borrow()
            .get(&key_hash)
            .copied()
    }

    #[inline]
    pub fn set_cached_node_range(
        &self, node: &'ast AstNode<'ast>, range: (DocCharOffset, DocCharOffset),
    ) {
        let key_hash = Self::pack_node_key(node);
        self.layout_cache
            .node_range
            .borrow_mut()
            .insert(key_hash, range);
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
