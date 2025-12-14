use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::{DocByteOffset, DocCharOffset, RangeExt as _, RangeIterExt};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::RangesExt as _;

pub(crate) mod wrap_layout;

impl<'ast> Editor {
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

    /// Returns a Vec of ranges that represent the given range split on newlines
    /// (based on source text). Behavior inspired by [`str::split`].
    pub fn range_split_newlines(
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

    pub fn selection_offset(&self) -> Option<DocCharOffset> {
        if self.buffer.current.selection.is_empty() {
            Some(self.buffer.current.selection.0)
        } else {
            None
        }
    }

    /// Returns the deepest container block node containing the offset.
    pub fn deepest_container_block_at_offset(
        &self, node: &'ast AstNode<'ast>, offset: DocCharOffset,
    ) -> &'ast AstNode<'ast> {
        for child in node.children() {
            if !child.data.borrow().value.is_container_block() {
                continue;
            }
            for line_idx in self.node_lines(child).iter() {
                let line = self.bounds.source_lines[line_idx];
                let node_line = self.node_line(child, line);
                if node_line.contains(offset, false, true) {
                    return self.deepest_container_block_at_offset(child, offset);
                }
            }
        }
        node
    }

    /// Returns the leaf block node containing the offset.
    pub fn leaf_block_at_offset(
        &self, node: &'ast AstNode<'ast>, offset: DocCharOffset,
    ) -> &'ast AstNode<'ast> {
        for child in node.children() {
            for line_idx in self.node_lines(child).iter() {
                let line = self.bounds.source_lines[line_idx];
                let node_line = self.node_line(child, line);
                if node_line.contains(offset, false, true) {
                    if child.data.borrow().value.is_leaf_block() {
                        return child;
                    } else {
                        return self.leaf_block_at_offset(child, offset);
                    }
                }
            }
        }
        node
    }

    pub fn line_at_offset(&self, offset: DocCharOffset) -> (DocCharOffset, DocCharOffset) {
        let (line_idx, _) = self.bounds.source_lines.find_containing(offset, true, true);
        self.bounds.source_lines[line_idx]
    }
}

#[allow(dead_code)]
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

impl<'ast> NodeValueExt for AstNode<'ast> {
    fn is_leaf_block(&self) -> bool {
        self.data.borrow().value.is_leaf_block()
    }

    fn is_container_block(&self) -> bool {
        self.data.borrow().value.is_container_block()
    }

    fn is_inline(&self) -> bool {
        self.data.borrow().value.is_inline()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn range_lines_char_no_newline() {
        let text = "*";
        let md = Editor::test(text);

        let lines = md.range_split_newlines((0.into(), text.len().into()));

        // Should produce 1 range for the entire text since there's no newline
        assert_eq!(lines, vec![(0.into(), 1.into())]);
    }

    #[test]
    fn range_lines_char_newline() {
        let text = "*\n";
        let md = Editor::test(text);

        let lines = md.range_split_newlines((0.into(), text.len().into()));

        // Should produce 2 ranges - one for "*" and one for empty line after "\n"
        assert_eq!(lines, vec![(0.into(), 1.into()), (2.into(), 2.into())]);
    }
}
