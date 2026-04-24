use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::{Byte, Grapheme, RangeExt as _, RangeIterExt};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::bounds::RangesExt as _;

pub(crate) mod wrap_layout;

/// Consume leading whitespace from `text` until the column position reaches
/// `target_columns`, returning the number of graphemes consumed. Per
/// CommonMark/GFM §2.2, tabs in indent positions act as if expanded to
/// spaces with a tab stop of 4 columns (so a tab at column 0 = 4 spaces, at
/// column 1 = 3 spaces, etc.). Tabs are atomic — consumed wholesale even if
/// they advance past `target_columns` (you can't strip half a tab from
/// source). Used by container-block prefix-stripping (list items, task
/// items, blockquotes) where the parent owns the leading indent and the
/// renderer needs to skip it before showing the line content.
pub fn consume_indent_columns(text: &str, target_columns: usize) -> usize {
    let mut cols = 0;
    let mut graphemes = 0;
    for c in text.chars() {
        if cols >= target_columns {
            break;
        }
        match c {
            ' ' => {
                cols += 1;
                graphemes += 1;
            }
            '\t' => {
                let new_cols = (cols / 4 + 1) * 4;
                // Don't consume the tab if its column span straddles
                // the target — the remaining virtual columns belong to
                // whatever strip runs next (per CommonMark §2.2). A
                // tab stripped whole would over-consume and leave less
                // content indent than the spec requires.
                if new_cols > target_columns {
                    break;
                }
                cols = new_cols;
                graphemes += 1;
            }
            _ => break,
        }
    }
    graphemes
}

impl<'ast> MdRender {
    // wrappers because I'm tired of writing ".buffer.current.segs" all the time
    pub fn offset_to_byte(&self, i: Grapheme) -> Byte {
        self.buffer.current.segs.offset_to_byte(i)
    }

    pub fn range_to_byte(&self, i: (Grapheme, Grapheme)) -> (Byte, Byte) {
        self.buffer.current.segs.range_to_byte(i)
    }

    pub fn offset_to_char(&self, i: Byte) -> Grapheme {
        self.buffer.current.segs.offset_to_char(i)
    }

    pub fn range_to_char(&self, i: (Byte, Byte)) -> (Grapheme, Grapheme) {
        self.buffer.current.segs.range_to_char(i)
    }

    /// Byte→char conversion for byte offsets that may not land on a grapheme
    /// boundary. Snaps each endpoint up to the next boundary.
    ///
    /// `Grapheme` indexes graphemes; the strict `range_to_char` panics on
    /// any byte that isn't a grapheme start (no corresponding char). That's the
    /// right behavior when bytes come from the buffer's own segs or round-trip
    /// through a `Grapheme` — a panic there is a real bug, not data we
    /// should paper over.
    ///
    /// Use this version *only* when the byte source promises codepoint
    /// boundaries but not grapheme boundaries — i.e. cosmic-text glyph byte
    /// positions and comrak sourcepos. Both index by codepoint and will hand
    /// us positions inside a cluster whenever an extending codepoint
    /// (Devanagari vowel sign, ZWJ, variation selector, virama) sits at a
    /// rendering or parsing seam. The strict converter has no recourse for
    /// these; rounding up is the only well-defined answer that keeps the
    /// editor usable on Indic / emoji text.
    ///
    /// Snapping consistently to ceil gives every cluster exactly one home: it
    /// belongs to whichever row/section's *end* boundary crosses it, and the
    /// next row/section's *start* lands at or past the cluster's far edge — no
    /// double-counting, no gaps.
    pub fn range_to_char_ceil(&self, i: (Byte, Byte)) -> (Grapheme, Grapheme) {
        let segs = &self.buffer.current.segs;
        (segs.byte_to_char_ceil(i.0), segs.byte_to_char_ceil(i.1))
    }

    pub fn last_cursor_position(&self) -> Grapheme {
        self.buffer.current.segs.last_cursor_position()
    }

    /// Returns a Vec of ranges that represent the given range split on newlines
    /// (based on source text). Behavior inspired by [`str::split`].
    pub fn range_split_newlines(&self, range: (Grapheme, Grapheme)) -> Vec<(Grapheme, Grapheme)> {
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

            // Find the end of this line. CommonMark line endings: \n, \r\n, or
            // bare \r (CR not followed by LF). Must match comrak's line
            // counting — if we missed a separator it splits on, our
            // `source_lines` wouldn't align with comrak's sourcepos line
            // numbers.
            while pos < bytes.len() {
                if bytes[pos] == b'\n' {
                    let line_range = (base_offset + start_pos, base_offset + pos);
                    result.push(self.range_to_char(line_range));
                    pos += 1;
                    start_pos = pos;
                    break;
                } else if bytes[pos] == b'\r' {
                    let line_range = (base_offset + start_pos, base_offset + pos);
                    result.push(self.range_to_char(line_range));
                    pos += if pos + 1 < bytes.len() && bytes[pos + 1] == b'\n' { 2 } else { 1 };
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

    pub fn selection_offset(&self) -> Option<Grapheme> {
        if self.buffer.current.selection.is_empty() {
            Some(self.buffer.current.selection.0)
        } else {
            None
        }
    }

    /// Returns the deepest container block node containing the offset.
    pub fn deepest_container_block_at_offset(
        &self, node: &'ast AstNode<'ast>, offset: Grapheme,
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
        &self, node: &'ast AstNode<'ast>, offset: Grapheme,
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

    pub fn line_at_offset(&self, offset: Grapheme) -> (Grapheme, Grapheme) {
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
    #[test]
    fn range_lines_char_no_newline() {
        let text = "*";
        let md = crate::tab::markdown_editor::MdRender::test(text);
        let lines = md.range_split_newlines((0.into(), text.len().into()));
        assert_eq!(lines, vec![(0.into(), 1.into())]);
    }

    #[test]
    fn range_lines_char_newline() {
        let text = "*\n";
        let md = crate::tab::markdown_editor::MdRender::test(text);
        let lines = md.range_split_newlines((0.into(), text.len().into()));
        assert_eq!(lines, vec![(0.into(), 1.into()), (2.into(), 2.into())]);
    }
}
