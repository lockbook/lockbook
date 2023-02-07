use crate::offset_types::DocCharOffset;
use crate::unicode_segs::UnicodeSegs;
use std::ops::Range;

#[derive(Default, Debug)]
pub struct Buffer {
    pub raw: String,
}

impl From<&str> for Buffer {
    fn from(value: &str) -> Self {
        Self { raw: value.into() }
    }
}

impl Buffer {
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    pub fn len(&self) -> usize {
        self.raw.len()
    }

    pub fn replace_range(
        &mut self, range: Range<DocCharOffset>, replacement: &str, segs: &UnicodeSegs,
    ) {
        self.raw.replace_range(
            Range {
                start: segs.char_offset_to_byte(range.start).0,
                end: segs.char_offset_to_byte(range.end).0,
            },
            replacement,
        );
    }
}
