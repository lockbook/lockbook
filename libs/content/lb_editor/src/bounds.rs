use crate::ast::{Ast, AstTextRangeType};
use crate::buffer::SubBuffer;
use crate::offset_types::{DocByteOffset, DocCharOffset, RelByteOffset};
use crate::Editor;
use std::collections::HashSet;
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
                        // whitespace-only sequences don't count as words
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
pub struct Paragraphs {
    pub paragraphs: Vec<(DocCharOffset, DocCharOffset)>,
}

pub fn calc_paragraphs(buffer: &SubBuffer, ast: &Ast) -> Paragraphs {
    let mut result = vec![];

    let captured_newlines = {
        let mut captured_newlines = HashSet::new();
        for text_range in ast.iter_text_ranges() {
            match text_range.range_type {
                AstTextRangeType::Head | AstTextRangeType::Tail => {
                    // newlines in syntax sequences don't break paragraphs
                    let range_start_byte = buffer.segs.offset_to_byte(text_range.range.0);
                    captured_newlines.extend(buffer[text_range.range].match_indices('\n').map(
                        |(idx, _)| {
                            buffer
                                .segs
                                .offset_to_char(range_start_byte + RelByteOffset(idx))
                        },
                    ))
                }
                AstTextRangeType::Text => {}
            }
        }
        captured_newlines
    };

    let mut prev_char_offset = DocCharOffset(0);
    for (byte_offset, _) in (buffer.text.to_string() + "\n").match_indices('\n') {
        let char_offset = buffer.segs.offset_to_char(DocByteOffset(byte_offset));
        if captured_newlines.contains(&char_offset) {
            continue;
        }

        // note: paragraphs can be empty
        result.push((prev_char_offset, char_offset));

        prev_char_offset = char_offset + 1 // skip the matched newline;
    }

    Paragraphs { paragraphs: result }
}

impl Editor {
    pub fn print_paragraphs(&self) {
        println!(
            "paragraphs: {:?}",
            self.paragraphs
                .paragraphs
                .iter()
                .map(|&range| self.buffer.current[range].to_string())
                .collect::<Vec<_>>()
        );
    }
}
