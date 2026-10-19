//! Small, stateless CPU helpers for document cards. No editor or GPU state is touched.

use unicode_segmentation::UnicodeSegmentation;

pub fn cursor_utf16(text: &str, grapheme: usize) -> usize {
    text.graphemes(true)
        .take(grapheme)
        .map(|g| g.encode_utf16().count())
        .sum()
}
