use crate::ast::{Ast, AstTextRangeType};
use crate::buffer::SubBuffer;
use crate::galleys::Galleys;
use crate::offset_types::{DocCharOffset, RelByteOffset, RelCharOffset};
use crate::Editor;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub struct Words {
    pub words: Vec<(DocCharOffset, DocCharOffset)>,
}

pub fn calc_words(buffer: &SubBuffer, ast: &Ast) -> Words {
    let mut result = vec![];

    for text_range in ast.iter_text_ranges() {
        match text_range.range_type {
            AstTextRangeType::Head | AstTextRangeType::Tail => {} // syntax sequences don't count as words
            AstTextRangeType::Text => {
                let mut prev_char_offset = text_range.range.0;
                let mut prev_word = "";
                for (byte_offset, word) in
                    (buffer[text_range.range].to_string() + " ").split_word_bound_indices()
                {
                    let char_offset = buffer.segs.offset_to_char(
                        buffer.segs.offset_to_byte(text_range.range.0) + RelByteOffset(byte_offset),
                    );

                    if !prev_word.trim().is_empty() {
                        result.push((prev_char_offset, char_offset));
                    }

                    prev_char_offset = char_offset;
                    prev_word = word;
                }
            }
        }
    }

    Words { words: result }
}

impl Editor {
    pub fn print_words(&self) {
        println!(
            "words: {:?}",
            self.words
                .words
                .iter()
                .map(|&range| self.buffer.current[range].to_string())
                .collect::<Vec<_>>()
        );
    }
}

#[derive(Default)]
pub struct Lines {
    pub lines: Vec<(DocCharOffset, DocCharOffset)>,
}

pub fn calc_lines(buffer: &SubBuffer, galleys: &Galleys) -> Vec<(DocCharOffset, DocCharOffset)> {
    todo!()
}

#[derive(Default)]
pub struct Paragraphs {
    pub paragraphs: Vec<(DocCharOffset, DocCharOffset)>,
}

pub fn calc_paragraphs(buffer: &SubBuffer, ast: &Ast) -> Vec<(DocCharOffset, DocCharOffset)> {
    todo!()
}
