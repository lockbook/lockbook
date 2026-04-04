use std::cell::RefCell;
use std::sync::{Arc, Mutex, RwLock};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format};

use comrak::nodes::{AstNode, NodeHeading, NodeLink, NodeValue};
use egui::ahash::HashMap;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{
    DocCharOffset, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_editor::widget::inline::html_inline::FOLD_TAG;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;

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
            NodeValue::FrontMatter(_) => self.width - 2. * self.layout.margin,
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => indented_width(),
            NodeValue::BlockQuote => indented_width(),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.width - 2. * self.layout.margin,
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

    pub fn height(
        &self, node: &'ast AstNode<'ast>, siblings: &[&'ast AstNode<'ast>],
    ) -> f32 {
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
        if self.hidden_by_fold(node, siblings) {
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

    pub(crate) fn show_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        siblings: &[&'ast AstNode<'ast>],
    ) {
        let ui = &mut self.node_ui(ui, node);

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
        if self.hidden_by_fold(node, siblings) {
            return;
        }

        let value = &node.data.borrow().value;
        let sp = &node.data.borrow().sourcepos;
        match value {
            NodeValue::FrontMatter(_) => self.show_front_matter(ui, node, top_left),
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(node_alert) => self.show_alert(ui, node, top_left, node_alert, siblings),
            NodeValue::BlockQuote => self.show_block_quote(ui, node, top_left, siblings),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.show_document(ui, node, top_left),
            NodeValue::FootnoteDefinition(_) => self.show_footnote_definition(ui, node, top_left),
            NodeValue::Item(_) => self.show_item(ui, node, top_left, siblings),
            NodeValue::List(_) => self.show_block_children(ui, node, top_left),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => self.show_table(ui, node, top_left),
            NodeValue::TableRow(is_header_row) => {
                self.show_table_row(ui, node, top_left, *is_header_row)
            }
            NodeValue::TaskItem(node_task_item) => {
                self.show_task_item(ui, node, top_left, node_task_item, siblings)
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
                self.show_heading(ui, node, top_left, *level, *setext, siblings)
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

    /// Returns the children of the given node. With footnotes disabled,
    /// comrak reports children in sourcepos order so no sorting is needed.
    pub fn sorted_children(&self, node: &'ast AstNode<'ast>) -> Vec<&'ast AstNode<'ast>> {
        node.children().collect()
    }

    /// Returns the siblings of the given node in sourcepos order.
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

    pub fn hidden_by_fold(
        &self, node: &'ast AstNode<'ast>, siblings: &[&'ast AstNode<'ast>],
    ) -> bool {
        if let Some(cached) = self.get_cached_hidden_by_fold(node) {
            return cached;
        }

        let result = self.compute_hidden_by_fold(node, siblings);
        self.set_cached_hidden_by_fold(node, result);
        result
    }

    fn compute_hidden_by_fold(
        &self, node: &'ast AstNode<'ast>, sorted_siblings: &[&'ast AstNode<'ast>],
    ) -> bool {
        // show only the first block in folded ancestor blocks
        if node.previous_sibling().is_some() {
            for ancestor in node.ancestors().skip(1) {
                if matches!(
                    &ancestor.data.borrow().value,
                    NodeValue::Item(_) | NodeValue::TaskItem(_)
                ) && !self.item_fold_reveal(ancestor, &self.sorted_siblings(ancestor))
                    && self.fold(ancestor).is_some()
                {
                    return true;
                }
            }
        }

        // show only the blocks that have no folded heading; headings with
        // another equal or more significant heading between them and the target
        // node don't count; headings intersecting the selection don't count
        let sibling_index = self.sibling_index(node, sorted_siblings);

        let mut most_significant_unfolded_heading =
            if let NodeValue::Heading(heading) = &node.data.borrow().value {
                heading.level
            } else {
                7 // max heading level + 1
            };
        for sibling in sorted_siblings[0..sibling_index].iter().rev() {
            if let NodeValue::Heading(heading) = &sibling.data.borrow().value {
                if heading.level < most_significant_unfolded_heading {
                    most_significant_unfolded_heading = heading.level;
                    if !self.heading_fold_reveal(sibling, sorted_siblings) && self.fold(sibling).is_some() {
                        // our node is contained by a folded, unrevealed heading
                        return true;
                    }
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
        NodeValue::Highlight => 42,
        NodeValue::ShortCode(_) => 43,
        NodeValue::Subtext => 44,
    }
}

pub struct LinePrefixCacheEntry {
    node_key_hash: u64,
    line: (DocCharOffset, DocCharOffset),
    value: (RelCharOffset, bool),
}

pub enum TitleState {
    Loading,
    Loaded(String),
    Failed,
}

#[derive(Default)]
pub struct LayoutCache {
    pub height: RefCell<Vec<CacheEntry<f32>>>,
    pub line_prefix_len: RefCell<Vec<LinePrefixCacheEntry>>,
    pub node_range: RefCell<HashMap<u64, (DocCharOffset, DocCharOffset)>>,
    pub hidden_by_fold: RefCell<Vec<CacheEntry<bool>>>,
    pub glyphon_buffers: RefCell<HashMap<GlyphonBufferKey, Arc<RwLock<glyphon::Buffer>>>>,
    pub link_titles: RefCell<HashMap<String, Arc<Mutex<TitleState>>>>,
}

#[derive(Hash, PartialEq, Eq)]
pub struct GlyphonBufferKey {
    pub text: String,
    pub font_size_bits: u32,
    pub line_height_bits: u32,
    pub width_bits: u32,
    pub family: FontFamily,
    pub bold: bool,
    pub italic: bool,
    pub color: [u8; 4],
}

impl GlyphonBufferKey {
    pub fn new(text: &str, font_size: f32, line_height: f32, width: f32, format: &Format) -> Self {
        Self {
            text: text.to_string(),
            font_size_bits: font_size.to_bits(),
            line_height_bits: line_height.to_bits(),
            width_bits: width.to_bits(),
            family: format.family.clone(),
            bold: format.bold,
            italic: format.italic,
            color: format.color.to_array(),
        }
    }
}

impl LayoutCache {
    /// Full clear for width/resize changes where everything must be recomputed.
    pub fn clear(&self) {
        self.height.borrow_mut().clear();
        self.line_prefix_len.borrow_mut().clear();
        self.node_range.borrow_mut().clear();
        self.hidden_by_fold.borrow_mut().clear();
        self.glyphon_buffers.borrow_mut().clear();
        // link_titles intentionally not cleared: fetched titles persist across layout invalidations
    }

    /// Invalidation for text changes. Position-keyed caches (height,
    /// hidden_by_fold) must be cleared because DocCharOffsets shift.
    /// Content-keyed caches (glyphon_buffers) are preserved since unchanged
    /// text still matches. Sourcepos-keyed caches (node_range,
    /// line_prefix_len) are cleared because the AST is re-parsed.
    pub fn invalidate_text_change(&self) {
        self.height.borrow_mut().clear();
        self.line_prefix_len.borrow_mut().clear();
        self.node_range.borrow_mut().clear();
        self.hidden_by_fold.borrow_mut().clear();
        // glyphon_buffers: content-addressed, preserved across text changes
    }

    /// Invalidates height entries affected by a selection change. A node's
    /// height depends on its reveal state (selection-dependent), so we evict
    /// nodes that intersect the old or new selection. We also evict ancestors
    /// of invalidated nodes (any entry whose range contains an evicted range)
    /// because their heights are sums of their children's heights.
    pub fn invalidate_selection_change(
        &self,
        old_selection: (DocCharOffset, DocCharOffset),
        new_selection: (DocCharOffset, DocCharOffset),
    ) {
        let mut cache = self.height.borrow_mut();

        // first pass: find ranges directly affected by the selection change
        let mut invalidated: Vec<(DocCharOffset, DocCharOffset)> = Vec::new();
        for entry in cache.iter() {
            if entry.range.intersects(&old_selection, true)
                || entry.range.intersects(&new_selection, true)
            {
                invalidated.push(entry.range);
            }
        }

        if invalidated.is_empty() {
            return;
        }

        // second pass: evict directly affected nodes and their ancestors
        cache.retain(|entry| {
            // directly affected
            if entry.range.intersects(&old_selection, true)
                || entry.range.intersects(&new_selection, true)
            {
                return false;
            }
            // ancestor of an affected node
            for inv in &invalidated {
                if entry.range.contains_range(inv, true, true) {
                    return false;
                }
            }
            true
        });
    }
}

impl Editor {
    pub fn upsert_glyphon_buffer(
        &self, text: &str, font_size: f32, line_height: f32, width: f32, format: &Format,
    ) -> Arc<RwLock<glyphon::Buffer>> {
        let font_system = self
            .ctx
            .data(|d| d.get_temp::<Arc<Mutex<glyphon::FontSystem>>>(egui::Id::NULL))
            .unwrap();

        let ppi = self.ctx.pixels_per_point();
        let font_size = font_size * ppi;
        let line_height = line_height * ppi;
        let width = width * ppi;
        let key = GlyphonBufferKey::new(text, font_size, line_height, width, format);
        let mut cache = self.layout_cache.glyphon_buffers.borrow_mut();
        cache
            .entry(key)
            .or_insert_with(|| {
                let attrs = glyphon::Attrs::new()
                    .family(match format.family {
                        FontFamily::Sans => glyphon::Family::SansSerif,
                        FontFamily::Mono => glyphon::Family::Monospace,
                        FontFamily::Icons => glyphon::Family::Name("Nerd Fonts Mono Symbols"),
                    })
                    .weight(if format.bold {
                        glyphon::Weight::BOLD
                    } else {
                        glyphon::Weight::NORMAL
                    })
                    .style(if format.italic {
                        glyphon::Style::Italic
                    } else {
                        glyphon::Style::Normal
                    });
                let metrics = glyphon::Metrics::new(font_size, line_height);
                let mut b = glyphon::Buffer::new(&mut font_system.lock().unwrap(), metrics);
                b.set_size(&mut font_system.lock().unwrap(), Some(width), None);
                let emoji_attrs =
                    glyphon::Attrs::new().family(glyphon::Family::Name("Twemoji Mozilla"));
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
                    &mut font_system.lock().unwrap(),
                    spans,
                    &attrs,
                    glyphon::Shaping::Advanced,
                    None,
                );
                b.shape_until_scroll(&mut font_system.lock().unwrap(), false);
                Arc::new(RwLock::new(b))
            })
            .clone()
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

    pub fn get_cached_hidden_by_fold(&self, node: &'ast AstNode<'ast>) -> Option<bool> {
        let range = self.node_range(node);
        self.layout_cache
            .hidden_by_fold
            .borrow()
            .binary_search_by(|entry| entry.range.cmp(&range))
            .ok()
            .map(|i| self.layout_cache.hidden_by_fold.borrow()[i].value)
    }

    pub fn set_cached_hidden_by_fold(&self, node: &'ast AstNode<'ast>, hidden: bool) {
        let range = self.node_range(node);
        let mut cache = self.layout_cache.hidden_by_fold.borrow_mut();
        match cache.binary_search_by(|entry| entry.range.cmp(&range)) {
            Ok(i) => cache[i].value = hidden,
            Err(i) => cache.insert(i, CacheEntry { range, value: hidden }),
        }
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
