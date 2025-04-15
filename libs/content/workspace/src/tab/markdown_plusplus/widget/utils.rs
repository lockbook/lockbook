use lb_rs::model::text::offset_types::{DocByteOffset, DocCharOffset};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl MarkdownPlusPlus {
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
}
