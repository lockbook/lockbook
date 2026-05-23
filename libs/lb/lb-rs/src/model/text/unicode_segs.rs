use super::offset_types::{Byte, Grapheme};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Default, Debug)]
pub struct UnicodeSegs {
    pub grapheme_indexes: Vec<Byte>,
}

pub fn calc(text: &str) -> UnicodeSegs {
    let mut result = UnicodeSegs::default();
    if !text.is_empty() {
        result
            .grapheme_indexes
            .extend(text.grapheme_indices(true).map(|t| Byte(t.0)));
    }
    result.grapheme_indexes.push(Byte(text.len()));
    result
}

impl UnicodeSegs {
    pub fn offset_to_byte(&self, i: Grapheme) -> Byte {
        if self.grapheme_indexes.is_empty() && i.0 == 0 {
            return Byte(0);
        }
        self.grapheme_indexes[i.0]
    }

    pub fn range_to_byte(&self, i: (Grapheme, Grapheme)) -> (Byte, Byte) {
        (self.offset_to_byte(i.0), self.offset_to_byte(i.1))
    }

    pub fn offset_to_char(&self, i: Byte) -> Grapheme {
        if self.grapheme_indexes.is_empty() && i.0 == 0 {
            return Grapheme(0);
        }

        Grapheme(self.grapheme_indexes.binary_search(&i).unwrap())
    }

    pub fn range_to_char(&self, i: (Byte, Byte)) -> (Grapheme, Grapheme) {
        (self.offset_to_char(i.0), self.offset_to_char(i.1))
    }

    /// Snap a byte offset down to the start of the grapheme containing it.
    /// Use for converting an *inclusive* byte position from a non-grapheme-
    /// aware source into a `Grapheme`.
    pub fn byte_to_char_floor(&self, b: Byte) -> Grapheme {
        match self.grapheme_indexes.binary_search(&b) {
            Ok(i) => Grapheme(i),
            Err(i) => Grapheme(i.saturating_sub(1)),
        }
    }

    /// Snap a byte offset up to the start of the next grapheme. Use for
    /// converting an *exclusive* byte position so the resulting range fully
    /// contains every grapheme any of its bytes belong to.
    pub fn byte_to_char_ceil(&self, b: Byte) -> Grapheme {
        match self.grapheme_indexes.binary_search(&b) {
            Ok(i) => Grapheme(i),
            Err(i) => Grapheme(i.min(self.grapheme_indexes.len().saturating_sub(1))),
        }
    }

    pub fn last_cursor_position(&self) -> Grapheme {
        Grapheme(self.grapheme_indexes.len() - 1)
    }
}
