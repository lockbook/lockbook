use crate::tab::markdown_editor::offset_types::{DocByteOffset, DocCharOffset};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Default, Debug)]
pub struct UnicodeSegs {
    pub grapheme_indexes: Vec<DocByteOffset>,
}

pub fn calc(text: &str) -> UnicodeSegs {
    let mut result = UnicodeSegs::default();
    if !text.is_empty() {
        result
            .grapheme_indexes
            .extend(text.grapheme_indices(true).map(|t| DocByteOffset(t.0)));
    }
    result.grapheme_indexes.push(DocByteOffset(text.len()));
    result
}

impl UnicodeSegs {
    pub fn offset_to_byte(&self, i: DocCharOffset) -> DocByteOffset {
        if self.grapheme_indexes.is_empty() && i.0 == 0 {
            return DocByteOffset(0);
        }
        self.grapheme_indexes[i.0]
    }

    pub fn range_to_byte(
        &self, i: (DocCharOffset, DocCharOffset),
    ) -> (DocByteOffset, DocByteOffset) {
        (self.offset_to_byte(i.0), self.offset_to_byte(i.1))
    }

    pub fn offset_to_char(&self, i: DocByteOffset) -> DocCharOffset {
        if self.grapheme_indexes.is_empty() && i.0 == 0 {
            return DocCharOffset(0);
        }

        DocCharOffset(self.grapheme_indexes.binary_search(&i).unwrap())
    }

    pub fn range_to_char(
        &self, i: (DocByteOffset, DocByteOffset),
    ) -> (DocCharOffset, DocCharOffset) {
        (self.offset_to_char(i.0), self.offset_to_char(i.1))
    }

    pub fn last_cursor_position(&self) -> DocCharOffset {
        DocCharOffset(self.grapheme_indexes.len() - 1)
    }
}
