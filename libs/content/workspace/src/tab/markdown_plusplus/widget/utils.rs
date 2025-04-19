use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::{DocByteOffset, DocCharOffset, RangeExt as _};

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
    // This entire fn, impressively, was written by Claude 3.7 Sonnet
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
            let start_pos = pos;

            // Find the end of this line
            while pos < bytes.len() {
                // Check for line endings
                if bytes[pos] == b'\n' {
                    // Found a newline
                    let line_range = (base_offset + start_pos, base_offset + pos);
                    result.push(self.range_to_char(line_range));
                    pos += 1; // Move past the \n
                    break;
                } else if pos + 1 < bytes.len() && bytes[pos] == b'\r' && bytes[pos + 1] == b'\n' {
                    // Found a CRLF
                    let line_range = (base_offset + start_pos, base_offset + pos);
                    result.push(self.range_to_char(line_range));
                    pos += 2; // Move past the \r\n
                    break;
                }

                pos += 1;
            }

            // If we reached the end without finding a line ending
            if pos == bytes.len() && start_pos < pos {
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

    /// Returns the range for the node - easy enough to do yourself but comes up
    /// so often.
    pub fn node_range(&self, node: &'ast AstNode<'ast>) -> (DocCharOffset, DocCharOffset) {
        self.sourcepos_to_range(node.data.borrow().sourcepos)
    }

    /// Returns the (inclusive, exclusive) range of lines that this node is sourced from.
    pub fn node_lines(&self, node: &'ast AstNode<'ast>) -> (usize, usize) {
        let range_lines = self.range_lines(self.node_range(node));

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

    /// Returns the first line, the whole first line, and nothing but the first
    /// line of the given node.
    pub fn node_first_line(&self, node: &'ast AstNode<'ast>) -> (DocCharOffset, DocCharOffset) {
        let range_lines = self.range_lines(self.node_range(node));

        let first_line = *range_lines.first().unwrap();
        let start_line_idx = self
            .bounds
            .source_lines
            .find_containing(first_line.start(), true, true)
            .start();
        self.bounds.source_lines[start_line_idx]
    }

    /// Returns true if the node intersects the current selection. Useful for
    /// checking if syntax should be revealed for a whole node.
    pub fn node_intersects_selection(&self, node: &'ast AstNode<'ast>) -> bool {
        self.node_range(node)
            .intersects(&self.buffer.current.selection, true)
    }

    /// Returns true if the line prefix intersects the current selection. Useful
    /// for checking if syntax should be revealed for one line of a multiline
    /// container block with per-line syntax, like block quotes.
    pub fn line_prefix_intersects_selection(
        &self, node: &'ast AstNode<'ast>, mut line: (DocCharOffset, DocCharOffset),
    ) -> bool {
        line.0 += self.line_prefix_len(node, line);
        line.intersects(&self.buffer.current.selection, true)
    }

    /// Returns the top y-coordinate of the given node.
    pub fn node_top_left(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut top_left = if let Some(parent) = node.parent() {
            self.node_top_left(parent)
        } else {
            0.0 // document / base case for all invocations
        };

        for sibling in self.sorted_siblings(node) {
            top_left += self.height(node)
        }

        top_left
    }

    /// Returns the top y-coordinate of the given source line. Useful for
    /// positioning container block syntax or annotations with attention to
    /// content height, so that, for instance, a bulleted list with a nested
    /// heading can vertically center the bullet on the larger-than-normal
    /// heading text like in GitHub.
    pub fn line_top_left() -> f32 {
        // find the node that determines the text format
        todo!()
    }

    /// Returns the preceding siblings of the given node in sourcepos order
    /// (unlike `node.preceding_siblings()`).
    pub fn preceding_siblings(&self, node: &'ast AstNode<'ast>) -> Vec<&'ast AstNode<'ast>> {
        let sorted_siblings = self.sorted_siblings(node);
        let node_idx = sorted_siblings
            .iter()
            .position(|n| n.data.borrow().sourcepos == node.data.borrow().sourcepos)
            .unwrap();
        sorted_siblings[..node_idx].to_vec()
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
