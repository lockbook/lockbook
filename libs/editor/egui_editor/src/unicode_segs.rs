use crate::buffer::Buffer;
use crate::offset_types::{DocByteOffset, DocCharOffset};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default, Debug)]
pub struct UnicodeSegs {
    pub grapheme_indexes: Vec<DocByteOffset>,
}

pub fn calc(buffer: &Buffer) -> UnicodeSegs {
    let mut result = UnicodeSegs::default();
    if !buffer.is_empty() {
        result.grapheme_indexes.extend(
            UnicodeSegmentation::grapheme_indices(buffer.raw.as_str(), true)
                .map(|t| DocByteOffset(t.0)),
        );
    }
    result.grapheme_indexes.push(DocByteOffset(buffer.len()));
    result
}

impl UnicodeSegs {
    pub fn char_offset_to_byte(&self, i: DocCharOffset) -> DocByteOffset {
        if self.grapheme_indexes.is_empty() && i.0 == 0 {
            return DocByteOffset(0);
        }
        self.grapheme_indexes[i.0]
    }

    pub fn byte_offset_to_char(&self, i: DocByteOffset) -> DocCharOffset {
        if self.grapheme_indexes.is_empty() && i.0 == 0 {
            return DocCharOffset(0);
        }

        DocCharOffset(self.grapheme_indexes.binary_search(&i).unwrap())
    }

    pub fn last_cursor_position(&self) -> DocCharOffset {
        DocCharOffset(self.grapheme_indexes.len() - 1)
    }
}
