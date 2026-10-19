//! Small, stateless CPU helpers for document cards. No editor or GPU state is touched.

use unicode_segmentation::UnicodeSegmentation;

pub fn cursor_utf16(text: &str, grapheme: usize) -> usize {
    text.graphemes(true)
        .take(grapheme)
        .map(|g| g.encode_utf16().count())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_cursor_conversion_preserves_extended_graphemes() {
        let text = "a👩🏽‍💻e\u{301}z";
        assert_eq!(cursor_utf16(text, 0), 0);
        assert_eq!(cursor_utf16(text, 2), "a👩🏽‍💻".encode_utf16().count());
        assert_eq!(cursor_utf16(text, 3), "a👩🏽‍💻e\u{301}".encode_utf16().count());
        assert_eq!(cursor_utf16(text, usize::MAX), text.encode_utf16().count());
    }
}
