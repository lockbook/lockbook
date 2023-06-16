use crate::buffer::SubBuffer;
use crate::galleys::Galleys;
use crate::offset_types::{DocByteOffset, DocCharOffset};
use std::iter;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub struct Bounds {
    pub words: Vec<(DocCharOffset, DocCharOffset)>,
    pub lines: Vec<(DocCharOffset, DocCharOffset)>,
    pub paragraphs: Vec<(DocCharOffset, DocCharOffset)>,
}

pub fn calc(buffer: &SubBuffer, galleys: &Galleys) -> Bounds {
    Bounds {
        words: calc_words(buffer, galleys),
        lines: calc_lines(buffer, galleys),
        paragraphs: calc_paragraphs(buffer, galleys),
    }
}

// todo: compute ast node head/tail and skip over
fn calc_words(buffer: &SubBuffer, galleys: &Galleys) -> Vec<(DocCharOffset, DocCharOffset)> {
    let mut result = vec![];

    let mut prev_char_offset = DocCharOffset(0);
    let mut prev_word = "";
    for (byte_offset, word) in buffer.text.split_word_bound_indices() {
        let char_offset = buffer.segs.offset_to_char(DocByteOffset(byte_offset));

        if !prev_word.trim().is_empty() {
            result.push((prev_char_offset, char_offset))
        }

        prev_char_offset = char_offset;
        prev_word = word;
    }

    result
}

fn calc_lines(buffer: &SubBuffer, galleys: &Galleys) -> Vec<(DocCharOffset, DocCharOffset)> {
    Default::default()
}

fn calc_paragraphs(buffer: &SubBuffer, galleys: &Galleys) -> Vec<(DocCharOffset, DocCharOffset)> {
    Default::default()
}

impl Bounds {}
