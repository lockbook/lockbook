use std::cell::RefCell;

use comrak::nodes::{AstNode, NodeHeading, NodeLink, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{
    DocCharOffset, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_editor::widget::BLOCK_SPACING;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;

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

                height += self.height_text_line(
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
                self.show_text_line(
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

    /// Returns the children of the given node in sourcepos order.
    pub fn sorted_children(&self, node: &'ast AstNode<'ast>) -> Vec<&'ast AstNode<'ast>> {
        let mut children = Vec::new();
        children.extend(node.children());
        children.sort_by(|a, b| {
            a.data
                .borrow()
                .sourcepos
                .start
                .line
                .cmp(&b.data.borrow().sourcepos.start.line)
        });
        children
    }

    /// Returns the siblings of the given node in sourcepos order (unlike
    /// `node.siblings()`).
    pub fn sorted_siblings(&self, node: &'ast AstNode<'ast>) -> Vec<&'ast AstNode<'ast>> {
        let mut preceding_siblings = node.preceding_siblings();
        preceding_siblings.next().unwrap(); // "Call .next().unwrap() once on the iterator to skip the node itself."

        let mut following_siblings = node.following_siblings();
        following_siblings.next().unwrap(); // "Call .next().unwrap() once on the iterator to skip the node itself."

        let mut siblings = Vec::new();
        siblings.extend(preceding_siblings);
        siblings.push(node);
        siblings.extend(following_siblings);
        siblings.sort_by(|a, b| {
            a.data
                .borrow()
                .sourcepos
                .start
                .line
                .cmp(&b.data.borrow().sourcepos.start.line)
        });
        siblings
    }

    pub fn sibling_index(
        &self, node: &'ast AstNode<'ast>, sorted_siblings: &[&'ast AstNode<'ast>],
    ) -> usize {
        let range = self.node_range(node);
        let this_sibling_index = sorted_siblings
            .iter()
            .position(|sibling| self.node_range(sibling) == range)
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
}

#[derive(Default)]
pub struct CacheEntry<T> {
    range: (DocCharOffset, DocCharOffset),
    value: T,
}

pub struct LinePrefixCacheEntry {
    node_ptr: usize,
    line: (DocCharOffset, DocCharOffset),
    value: (RelCharOffset, bool),
}

#[derive(Default)]
pub struct LayoutCache {
    pub height: RefCell<Vec<CacheEntry<f32>>>,
    pub line_prefix_len: RefCell<Vec<LinePrefixCacheEntry>>,
}

impl LayoutCache {
    pub fn clear(&self) {
        self.height.borrow_mut().clear();
        self.line_prefix_len.borrow_mut().clear();
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
        let node_ptr = node as *const _ as usize;
        self.layout_cache
            .line_prefix_len
            .borrow()
            .binary_search_by(|entry| entry.node_ptr.cmp(&node_ptr).then(entry.line.cmp(&line)))
            .ok()
            .map(|i| self.layout_cache.line_prefix_len.borrow()[i].value)
    }

    pub fn set_cached_line_prefix_len(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        value: (RelCharOffset, bool),
    ) {
        let node_ptr = node as *const _ as usize;
        let mut cache = self.layout_cache.line_prefix_len.borrow_mut();
        match cache
            .binary_search_by(|entry| entry.node_ptr.cmp(&node_ptr).then(entry.line.cmp(&line)))
        {
            Ok(i) => cache[i].value = value,
            Err(i) => cache.insert(i, LinePrefixCacheEntry { node_ptr, line, value }),
        }
    }
}
