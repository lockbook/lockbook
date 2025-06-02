use comrak::nodes::{AstNode, NodeCodeBlock, NodeValue};
use lb_rs::model::text::offset_types::{
    DocByteOffset, DocCharOffset, RangeExt as _, RangeIterExt as _,
};

use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    // wrappers because I'm tired of writing ".buffer.current.segs" all the time
    pub fn offset_to_byte(&self, i: DocCharOffset) -> DocByteOffset {
        self.buffer.current.segs.offset_to_byte(i)
    }

    pub fn range_to_byte(
        &self, i: (DocCharOffset, DocCharOffset),
    ) -> (DocByteOffset, DocByteOffset) {
        self.buffer.current.segs.range_to_byte(i)
    }

    pub fn offset_to_char(&self, i: DocByteOffset) -> DocCharOffset {
        self.buffer.current.segs.offset_to_char(i)
    }

    pub fn range_to_char(
        &self, i: (DocByteOffset, DocByteOffset),
    ) -> (DocCharOffset, DocCharOffset) {
        self.buffer.current.segs.range_to_char(i)
    }

    pub fn last_cursor_position(&self) -> DocCharOffset {
        self.buffer.current.segs.last_cursor_position()
    }

    // additional helpers
    /// Returns a range that represents the given range with leading and
    /// trailing whitespace removed (based on source text). Returns the empty
    /// range at the end of the given range if the whole range is whitespace.
    pub fn range_trim(
        &self, range: (DocCharOffset, DocCharOffset),
    ) -> (DocCharOffset, DocCharOffset) {
        let text = &self.buffer[range];
        let mut byte_range = self.range_to_byte(range);

        byte_range.0 += text.len() - text.trim_start().len();

        let text = &self.buffer[range];
        byte_range.1 -= text.len() - text.trim_end().len();

        self.range_to_char(byte_range)
    }

    /// Returns a range that represents the given range with leading whitespace
    /// removed (based on source text).
    pub fn range_trim_start(
        &self, range: (DocCharOffset, DocCharOffset),
    ) -> (DocCharOffset, DocCharOffset) {
        let text = &self.buffer[range];
        let mut byte_range = self.range_to_byte(range);

        byte_range.0 += text.len() - text.trim_start().len();

        self.range_to_char(byte_range)
    }

    /// Returns a range that represents the given range with trailing whitespace
    /// removed (based on source text).
    pub fn range_trim_end(
        &self, range: (DocCharOffset, DocCharOffset),
    ) -> (DocCharOffset, DocCharOffset) {
        let text = &self.buffer[range];
        let mut byte_range = self.range_to_byte(range);

        byte_range.1 -= text.len() - text.trim_end().len();

        self.range_to_char(byte_range)
    }

    /// Returns a Vec of ranges that represent the given range split on newlines
    /// (based on source text).
    pub fn range_lines(
        &self, range: (DocCharOffset, DocCharOffset),
    ) -> Vec<(DocCharOffset, DocCharOffset)> {
        let text = &self.buffer[range];
        let byte_range = self.range_to_byte(range);
        let base_offset = byte_range.0;

        // Special case for empty input
        if text.is_empty() {
            return vec![range];
        }

        let mut result = Vec::new();
        let bytes = text.as_bytes();
        let mut pos = 0;

        while pos < bytes.len() {
            let mut start_pos = pos;

            // Find the end of this line
            while pos < bytes.len() {
                // Check for line endings
                if bytes[pos] == b'\n' {
                    // Found a newline
                    let line_range = (base_offset + start_pos, base_offset + pos);
                    result.push(self.range_to_char(line_range));
                    pos += 1; // Move past the \n
                    start_pos = pos;
                    break;
                } else if pos + 1 < bytes.len() && bytes[pos] == b'\r' && bytes[pos + 1] == b'\n' {
                    // Found a CRLF
                    let line_range = (base_offset + start_pos, base_offset + pos);
                    result.push(self.range_to_char(line_range));
                    pos += 2; // Move past the \r\n
                    start_pos = pos;
                    break;
                }

                pos += 1;
            }

            // If we reached the end without finding a line ending
            if pos == bytes.len() {
                let line_range = (base_offset + start_pos, base_offset + pos);
                result.push(self.range_to_char(line_range));
            }
        }

        // For empty text or text with only newlines
        if result.is_empty() {
            result.push(range);
        }

        result
    }

    /// Returns a Vec of ranges that represent the given range, split on
    /// newlines, and expanded to include the whole first and last line.
    pub fn range_full_lines(
        &self, range: (DocCharOffset, DocCharOffset),
    ) -> Vec<(DocCharOffset, DocCharOffset)> {
        let mut range_lines = self.range_lines(range);

        if range_lines.is_empty() {
            return range_lines;
        }
        let first_line = *range_lines.first().unwrap();
        let last_line = *range_lines.last().unwrap();

        let start_line_idx = self
            .bounds
            .source_lines
            .find_containing(first_line.start(), true, true)
            .start();
        let start_line = self.bounds.source_lines[start_line_idx];

        let end_line_idx = self
            .bounds
            .source_lines
            .find_containing(last_line.end(), true, true)
            .start();
        let end_line = self.bounds.source_lines[end_line_idx];

        range_lines.first_mut().unwrap().0 = start_line.start();
        range_lines.last_mut().unwrap().1 = end_line.end();

        range_lines
    }

    /// Returns the range for the node.
    pub fn node_range(&self, node: &'ast AstNode<'ast>) -> (DocCharOffset, DocCharOffset) {
        let mut range = self.sourcepos_to_range(node.data.borrow().sourcepos);

        // hack: comrak's sourcepos's are unstable (and indeed broken) for some
        // nested block situations. clamping each range to its parent's prevents
        // the worst of the adverse consequences (e.g. double-rendering source
        // text).
        //
        // see also:
        // * https://github.com/kivikakk/comrak/issues/567
        // * https://github.com/kivikakk/comrak/issues/570
        if let Some(parent) = node.parent() {
            let parent_range = self.node_range(parent);
            range.0 = range.0.max(parent_range.0);
            range.1 = range.1.min(parent_range.1);
        }

        // hack: GFM spec says "Blank lines preceding or following an indented
        // code block are not included in it" and I have observed the behavior
        // for following lines to be incorrect in e.g. "    f\n"
        if let NodeValue::CodeBlock(NodeCodeBlock { fenced: false, .. }) = node.data.borrow().value
        {
            for line_idx in self.node_lines_impl(range).iter() {
                let line = self.bounds.source_lines[line_idx];
                let node_line = self.node_line(node, line);
                if self.buffer[node_line].chars().any(|c| !c.is_whitespace()) {
                    range.1 = line.end();
                }
            }
        }

        range
    }

    /// Returns the (inclusive, exclusive) range of lines that this node is sourced from.
    pub fn node_lines(&self, node: &'ast AstNode<'ast>) -> (usize, usize) {
        self.node_lines_impl(self.node_range(node))
    }

    fn node_lines_impl(&self, range: (DocCharOffset, DocCharOffset)) -> (usize, usize) {
        let range_lines = self.range_lines(range);

        let first_line = *range_lines.first().unwrap();
        let start_line_idx = self
            .bounds
            .source_lines
            .find_containing(first_line.start(), true, true)
            .start();

        let last_line = *range_lines.last().unwrap();
        let end_line_idx = self
            .bounds
            .source_lines
            .find_containing(last_line.end(), true, true)
            .end(); // note: preserves (inclusive, exclusive) behavior

        (start_line_idx, end_line_idx)
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

    /// Returns true if the node intersects the current selection. Useful for
    /// checking if syntax should be revealed for an inline node. Block nodes
    /// generally need additional consideration for optional indentation etc.
    pub fn node_intersects_selection(&self, node: &'ast AstNode<'ast>) -> bool {
        self.node_range(node)
            .intersects(&self.buffer.current.selection, true)
    }

    /// Returns true if the node's lines intersect the selection. Differs from
    /// node_lines_intersect_selection in cases where the selection intersects
    /// optional indentation, trailing whitespace, or the portion of a node's
    /// lines that are due to container blocks.
    pub fn node_lines_intersect_selection(&self, node: &'ast AstNode<'ast>) -> bool {
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);
            if node_line.intersects(&self.buffer.current.selection, true) {
                return true;
            }
        }
        false
    }

    pub fn children_in_range(
        &self, node: &'ast AstNode<'ast>, range: (DocCharOffset, DocCharOffset),
    ) -> Vec<&'ast AstNode<'ast>> {
        let mut children = Vec::new();
        for child in self.sorted_children(node) {
            if range.contains_range(&self.node_range(child), true, true) {
                children.push(child);
            }
        }
        children
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
}

pub trait NodeValueExt {
    fn is_leaf_block(&self) -> bool;
    fn is_container_block(&self) -> bool;
    fn is_inline(&self) -> bool;
}

impl NodeValueExt for NodeValue {
    fn is_leaf_block(&self) -> bool {
        match self {
            NodeValue::FrontMatter(_) | NodeValue::Raw(_) => false,

            // container_block
            NodeValue::Alert(_)
            | NodeValue::BlockQuote
            | NodeValue::DescriptionItem(_)
            | NodeValue::DescriptionList
            | NodeValue::Document
            | NodeValue::FootnoteDefinition(_)
            | NodeValue::Item(_)
            | NodeValue::List(_)
            | NodeValue::MultilineBlockQuote(_)
            | NodeValue::Table(_)
            | NodeValue::TableRow(_)
            | NodeValue::TaskItem(_) => false,

            // inline
            NodeValue::Image(_)
            | NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
            | NodeValue::Superscript
            | NodeValue::Text(_)
            | NodeValue::Underline
            | NodeValue::WikiLink(_) => false,

            // leaf_block
            NodeValue::CodeBlock(_)
            | NodeValue::DescriptionDetails
            | NodeValue::DescriptionTerm
            | NodeValue::Heading(_)
            | NodeValue::HtmlBlock(_)
            | NodeValue::Paragraph
            | NodeValue::TableCell
            | NodeValue::ThematicBreak => true,
        }
    }

    fn is_container_block(&self) -> bool {
        match self {
            NodeValue::FrontMatter(_) | NodeValue::Raw(_) => false,

            // container_block
            NodeValue::Alert(_)
            | NodeValue::BlockQuote
            | NodeValue::DescriptionItem(_)
            | NodeValue::DescriptionList
            | NodeValue::Document
            | NodeValue::FootnoteDefinition(_)
            | NodeValue::Item(_)
            | NodeValue::List(_)
            | NodeValue::MultilineBlockQuote(_)
            | NodeValue::Table(_)
            | NodeValue::TableRow(_)
            | NodeValue::TaskItem(_) => true,

            // inline
            NodeValue::Image(_)
            | NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
            | NodeValue::Superscript
            | NodeValue::Text(_)
            | NodeValue::Underline
            | NodeValue::WikiLink(_) => false,

            // leaf_block
            NodeValue::CodeBlock(_)
            | NodeValue::DescriptionDetails
            | NodeValue::DescriptionTerm
            | NodeValue::Heading(_)
            | NodeValue::HtmlBlock(_)
            | NodeValue::Paragraph
            | NodeValue::TableCell
            | NodeValue::ThematicBreak => false,
        }
    }

    fn is_inline(&self) -> bool {
        match self {
            NodeValue::FrontMatter(_) | NodeValue::Raw(_) => false,

            // container_block
            NodeValue::Alert(_)
            | NodeValue::BlockQuote
            | NodeValue::DescriptionItem(_)
            | NodeValue::DescriptionList
            | NodeValue::Document
            | NodeValue::FootnoteDefinition(_)
            | NodeValue::Item(_)
            | NodeValue::List(_)
            | NodeValue::MultilineBlockQuote(_)
            | NodeValue::Table(_)
            | NodeValue::TableRow(_)
            | NodeValue::TaskItem(_) => false,

            // inline
            NodeValue::Image(_)
            | NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
            | NodeValue::Superscript
            | NodeValue::Text(_)
            | NodeValue::Underline
            | NodeValue::WikiLink(_) => true,

            // leaf_block
            NodeValue::CodeBlock(_)
            | NodeValue::DescriptionDetails
            | NodeValue::DescriptionTerm
            | NodeValue::Heading(_)
            | NodeValue::HtmlBlock(_)
            | NodeValue::Paragraph
            | NodeValue::TableCell
            | NodeValue::ThematicBreak => false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn range_lines_char_no_newline() {
        let text = "*";
        let md = MarkdownPlusPlus::test(text);

        let lines = md.range_lines((0.into(), text.len().into()));

        // Should produce 1 range for the entire text since there's no newline
        assert_eq!(lines, vec![(0.into(), 1.into())]);
    }

    #[test]
    fn range_lines_char_newline() {
        let text = "*\n";
        let md = MarkdownPlusPlus::test(text);

        let lines = md.range_lines((0.into(), text.len().into()));

        // Should produce 2 ranges - one for "*" and one for empty line after "\n"
        assert_eq!(lines, vec![(0.into(), 1.into()), (2.into(), 2.into())]);
    }
}
