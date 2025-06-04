use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{
    DocCharOffset, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_plusplus::widget::INDENT;
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

pub(crate) mod alert;
pub(crate) mod block_quote;
pub(crate) mod document;
pub(crate) mod footnote_definition;
pub(crate) mod item;
pub(crate) mod list;
pub(crate) mod table;
pub(crate) mod table_row;
pub(crate) mod task_item;

impl<'ast> MarkdownPlusPlus {
    pub fn indent(&self, node: &'ast AstNode<'ast>) -> f32 {
        let value = &node.data.borrow().value;
        let sp = &node.data.borrow().sourcepos;
        match value {
            NodeValue::FrontMatter(_) => 0.,
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => INDENT,
            NodeValue::BlockQuote => INDENT,
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => 0.,
            NodeValue::FootnoteDefinition(_) => INDENT,
            NodeValue::Item(_) => INDENT,
            NodeValue::List(_) => 0., // indentation handled by items
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => 0.,
            NodeValue::TableRow(_) => 0.,
            NodeValue::TaskItem(_) => INDENT,

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
            NodeValue::CodeBlock(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::HtmlBlock(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::Paragraph => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::TableCell => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::ThematicBreak => unimplemented!("not a container block: {} {:?}", sp, value),
        }
    }

    // the height of a block that contains blocks is the sum of the heights of the blocks it contains
    pub fn block_children_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut height_sum = 0.0;
        for child in node.children() {
            height_sum += self.block_pre_spacing_height(child);
            height_sum += self.height(child);
            height_sum += self.block_post_spacing_height(child);
        }
        height_sum
    }

    // blocks are stacked vertically
    pub fn show_block_children(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let mut children: Vec<_> = node.children().collect();
        children.sort_by_key(|child| child.data.borrow().sourcepos);
        for child in children {
            // add pre-spacing
            let pre_spacing = self.block_pre_spacing_height(child);
            self.show_block_pre_spacing(ui, child, top_left);
            top_left.y += pre_spacing;

            // add block
            let child_height = self.height(child);
            self.show_block(ui, child, top_left);
            top_left.y += child_height;

            // add post-spacing
            let post_spacing = self.block_post_spacing_height(child);
            self.show_block_post_spacing(ui, child, top_left);
            top_left.y += post_spacing;
        }
    }

    /// How many leading characters on this line belong to the given node and
    /// its ancestors?
    // "It is tempting to think of this in terms of columns: the continuation
    // blocks must be indented at least to the column of the first
    // non-whitespace character after the list marker. However, that is not
    // quite right. The spaces after the list marker determine how much relative
    // indentation is needed. Which column this indentation reaches will depend
    // on how the list item is embedded in other constructions, as shown by this
    // example:
    //
    //    > > 1.  one
    // >>
    // >>     two
    //
    // Here two occurs in the same column as the list marker 1., but is actually
    // contained in the list item, because there is sufficient indentation after
    // the last containing blockquote marker."
    //
    // https://github.github.com/gfm/#list-items
    pub fn line_prefix_len(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> RelCharOffset {
        let parent = || node.parent().unwrap();
        let parent_line_prefix_len = || self.line_prefix_len(parent(), line);

        let value = &node.data.borrow().value;
        let sp = &node.data.borrow().sourcepos;
        match value {
            NodeValue::FrontMatter(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Raw(_) => unimplemented!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => self.line_prefix_len_alert(node, line),
            NodeValue::BlockQuote => self.line_prefix_len_block_quote(node, line),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => 0.into(),
            NodeValue::FootnoteDefinition(_) => {
                self.line_prefix_len_footnote_definition(node, line)
            }
            NodeValue::Item(node_list) => self.line_prefix_len_item(node, line, node_list),
            NodeValue::List(_) => parent_line_prefix_len(),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => parent_line_prefix_len(),
            NodeValue::TableRow(_) => parent_line_prefix_len(),
            NodeValue::TaskItem(_) => self.line_prefix_len_task_item(node, line),

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
            NodeValue::CodeBlock(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::HtmlBlock(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::Paragraph => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::TableCell => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::ThematicBreak => unimplemented!("not a container block: {} {:?}", sp, value),
        }
    }

    /// returns true if the syntax for a container block should be revealed
    pub fn reveal(&self, node: &'ast AstNode<'ast>) -> bool {
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];

            let line_prefix = (line.start(), line.start() + self.line_prefix_len(node, line));
            if line_prefix.is_empty() {
                continue;
            }

            let selection = self.buffer.current.selection;
            if line_prefix.intersects(&selection, false) {
                // line prefix contains some part of the selection
                return true;
            }
            if selection.end() == line_prefix.start() {
                // start of line prefix is inclusive
                return true;
            }
            if self.buffer[line_prefix].chars().all(|c| c.is_whitespace())
                && selection.start() == line_prefix.end()
            {
                // line prefix contains only whitespace: end of line prefix is inclusive
                // this improves the editing experience for list items
                return true;
            }
        }
        false
    }

    /// Returns whether the given line is one of the source lines of the given node
    pub fn node_contains_line(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> bool {
        let first_line = self.node_first_line(node);
        let last_line = self.node_last_line(node);

        (first_line.start(), last_line.end()).contains_range(&line, true, true)
    }

    /// Returns the row height of a line in the given node, even if that node is
    /// a container block.
    pub fn node_line_row_height(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        // leaf blocks and inlines
        if node.data.borrow().value.contains_inlines() || !node.data.borrow().value.block() {
            return self.row_height(node);
        }

        // container blocks only
        for child in node.children() {
            if self.node_contains_line(child, line) {
                return self.node_line_row_height(child, line);
            }
        }

        self.row_height(node)
    }
}
