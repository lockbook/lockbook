use crate::ast::{Ast, AstTextRangeType};
use crate::buffer::SubBuffer;
use crate::galleys::Galleys;
use crate::input::canonical::Bound;
use crate::offset_types::{DocByteOffset, DocCharOffset, RangeExt, RelByteOffset};
use crate::Editor;
use egui::epaint::text::cursor::RCursor;
use std::collections::HashSet;
use unicode_segmentation::UnicodeSegmentation;

type Words = Vec<(DocCharOffset, DocCharOffset)>;
type Lines = Vec<(DocCharOffset, DocCharOffset)>;
type Paragraphs = Vec<(DocCharOffset, DocCharOffset)>;

/// Represents bounds of various text regions in the buffer. Region bounds are inclusive on both sides. Regions do not
/// overlap, have region.0 <= region.1, and are sorted. Character and doc regions are not stored explicitly but can be
/// inferred from the other regions.
#[derive(Debug, Default)]
pub struct Bounds {
    pub words: Words,
    pub lines: Lines,

    /// Paragraphs consist of all rendered text, excluding the newlines that usually delimit them. Every valid cursor
    /// position is in some possibly-empty paragraph (inclusive of start/end).
    pub paragraphs: Paragraphs,
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

    result
}

pub fn calc_lines(galleys: &Galleys) -> Lines {
    let mut result = vec![];
    let galleys = galleys;
    for (galley_idx, galley) in galleys.galleys.iter().enumerate() {
        let galley = &galley.galley;
        for (row_idx, _) in galley.rows.iter().enumerate() {
            let start_cursor = galley.from_rcursor(RCursor { row: row_idx, column: 0 });
            let row_start = galleys.char_offset_by_galley_and_cursor(galley_idx, &start_cursor);
            let end_cursor = galley.cursor_end_of_row(&start_cursor);
            let row_end = galleys.char_offset_by_galley_and_cursor(galley_idx, &end_cursor);
            result.push((row_start, row_end))
        }
    }

    result
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

    result
}

impl Bounds {
    /// Returns the range with start < char_offset <= end, or None if there's no such range.
    fn range_before(
        ranges: &[(DocCharOffset, DocCharOffset)], char_offset: DocCharOffset,
    ) -> Option<usize> {
        ranges
            .iter()
            .enumerate()
            .rev()
            .find(|(_, &range)| range.start() < char_offset)
            .map(|(idx, _)| idx)
    }

    /// Returns the range with start <= char_offset < end, or None if there's no such range.
    fn range_after(
        ranges: &[(DocCharOffset, DocCharOffset)], char_offset: DocCharOffset,
    ) -> Option<usize> {
        ranges
            .iter()
            .enumerate()
            .find(|(_, &range)| char_offset < range.end())
            .map(|(idx, _)| idx)
    }
}

impl DocCharOffset {
    /// Returns the start or end of the range in the direction of `backwards` from offset `self`. If `jump` is true,
    /// `self` will not be at the boundary of the result in the direction of `backwards` (e.g. alt+left/right),
    /// otherwise it will be (e.g. cmd+left/right). For instance, if `jump` is true, advancing beyond the first or last
    /// range in the doc will return None, otherwise it will return the first or last range in the doc.
    fn range_bound(
        self, bound: Bound, backwards: bool, jump: bool, bounds: &Bounds,
    ) -> Option<(Self, Self)> {
        let ranges = match bound {
            Bound::Char => {
                return self.range_bound_char(backwards, jump, bounds);
            }
            Bound::Word => &bounds.words,
            Bound::Line => &bounds.lines,
            Bound::Paragraph => &bounds.paragraphs,
            Bound::Doc => {
                return Some((
                    bounds
                        .paragraphs
                        .first()
                        .map(|(start, _)| *start)
                        .unwrap_or(DocCharOffset(0)),
                    bounds
                        .paragraphs
                        .last()
                        .map(|(_, end)| *end)
                        .unwrap_or(DocCharOffset(0)),
                ));
            }
        };

        let range_before = Bounds::range_before(ranges, self);
        let range_after = Bounds::range_after(ranges, self);

        if jump {
            if backwards {
                range_before.map(|range_before| ranges[range_before])
            } else {
                range_after.map(|range_after| ranges[range_after])
            }
        } else {
            match (range_before, range_after) {
                // there are no ranges
                (None, None) => None,
                // before or at the start of the first range
                (None, Some(_)) => {
                    let first_range = ranges[0];
                    if self < first_range.start() {
                        // a cursor before the first range is considered at the start of the first range
                        first_range
                            .start()
                            .range_bound(bound, backwards, jump, bounds)
                    } else {
                        // self == first_range.start() because otherwise we'd have a range before
                        if backwards && jump {
                            // jump backwards off the edge from the start of the first range
                            None
                        } else {
                            // first range
                            Some(first_range)
                        }
                    }
                }
                // after or at the end of the last range
                (Some(_), None) => {
                    let last_range = ranges[ranges.len() - 1];
                    if self > last_range.end() {
                        // a cursor after the last range is considered at the end of the last range
                        last_range.end().range_bound(bound, backwards, jump, bounds)
                    } else {
                        // self == last_range.end() because otherwise we'd have a range after
                        if !backwards && jump {
                            // jump forwards off the edge from the end of the last range
                            None
                        } else {
                            // last range
                            Some(last_range)
                        }
                    }
                }
                // inside a range (not at bounds)
                (Some(range_before), Some(range_after)) if range_before == range_after => {
                    Some(ranges[range_before])
                }
                // at range bounds or between ranges
                (Some(range_before_idx), Some(range_after_idx)) => {
                    let range_before = ranges[range_before_idx];
                    let range_after = ranges[range_after_idx];
                    if range_before_idx + 1 != range_after_idx {
                        // in an empty range
                        Some((self, self))
                    } else if range_before.end() == range_after.start() {
                        // at bounds of two nonempty ranges
                        // range is nonempty because ranges cannot both be empty and touch a range before/after
                        if backwards {
                            Some(range_before)
                        } else {
                            Some(range_after)
                        }
                    } else if self == range_before.end() {
                        // at end of range before
                        // range before is nonempty because Bounds::range_before does not consider the range (offset, offset) to be before offset
                        Some(range_before)
                    } else if self == range_after.start() {
                        // at start of range after
                        // range after is nonempty because Bounds::range_after does not consider the range (offset, offset) to be after offset
                        Some(range_after)
                    } else {
                        // between ranges
                        if backwards {
                            Some(range_before)
                        } else {
                            Some(range_after)
                        }
                    }
                }
            }
        }
    }

    /// Returns the range in the direction of `backwards` from offset `self` representing a single character. If a
    /// range is returned, it's either a single character in a paragraph or a nonempty range between paragraphs. If
    /// `jump` is true, advancing beyond the first or last character in the doc will return None, otherwise it will
    /// return the first or last character in the doc.
    fn range_bound_char(
        self, backwards: bool, jump: bool, bounds: &Bounds,
    ) -> Option<(Self, Self)> {
        let paragraph_before = Bounds::range_before(&bounds.paragraphs, self);
        let paragraph_after = Bounds::range_after(&bounds.paragraphs, self);

        match (paragraph_before, paragraph_after) {
            // there are no paragraphs
            (None, None) => None,
            // before or at start of the first paragraph
            (None, Some(paragraph_after)) => {
                let first_paragraph = bounds.paragraphs[0];
                let paragraph_after = bounds.paragraphs[paragraph_after];
                if self < first_paragraph.start() {
                    // a cursor before the first paragraph is considered at the start of the first paragraph
                    first_paragraph
                        .start()
                        .range_bound_char(backwards, jump, bounds)
                } else {
                    // self == first_paragraph.start() because otherwise we'd have a paragraph before
                    if backwards && jump {
                        // jump backwards off the edge from the start of the first paragraph
                        None
                    } else if first_paragraph.is_empty() {
                        // nonempty range between paragraphs
                        // paragraph after is not first_paragraph because Bounds::range_after does not consider the range (offset, offset) to be after offset
                        // range is nonempty because paragraphs cannot both be empty and touch a paragraph before/after
                        Some((first_paragraph.start(), paragraph_after.start()))
                    } else {
                        // first character of the first paragraph
                        Some((first_paragraph.start(), first_paragraph.start() + 1))
                    }
                }
            }
            // after or at end of the last paragraph
            (Some(paragraph_before), None) => {
                let last_paragraph = bounds.paragraphs[bounds.paragraphs.len() - 1];
                let paragraph_before = bounds.paragraphs[paragraph_before];
                if self > last_paragraph.end() {
                    // a cursor after the last paragraph is considered at the end of the last paragraph
                    last_paragraph
                        .end()
                        .range_bound_char(backwards, jump, bounds)
                } else {
                    // self == last_paragraph.end() because otherwise we'd have a paragraph after
                    if !backwards && jump {
                        // jump forwards off the edge from the end of the last paragraph
                        None
                    } else if last_paragraph.is_empty() {
                        // nonempty range between paragraphs
                        // paragraph before is not last_paragraph because Bounds::range_before does not consider the range (offset, offset) to be before offset
                        // range is nonempty because paragraphs cannot both be empty and touch a paragraph before/after
                        Some((paragraph_before.end(), last_paragraph.end()))
                    } else {
                        // last character of the last paragraph
                        Some((last_paragraph.end() - 1, last_paragraph.end()))
                    }
                }
            }
            // inside a paragraph (not at bounds)
            (Some(paragraph_before), Some(paragraph_after))
                if paragraph_before == paragraph_after =>
            {
                if backwards {
                    Some((self - 1, self))
                } else {
                    Some((self, self + 1))
                }
            }
            // at paragraph bounds or between paragraphs
            (Some(paragraph_before_idx), Some(paragraph_after_idx)) => {
                let paragraph_before = bounds.paragraphs[paragraph_before_idx];
                let paragraph_after = bounds.paragraphs[paragraph_after_idx];
                if paragraph_before_idx + 1 != paragraph_after_idx {
                    // in an empty paragraph
                    if backwards {
                        Some((paragraph_before.end(), self))
                    } else {
                        Some((self, paragraph_after.start()))
                    }
                } else if paragraph_before.end() == paragraph_after.start() {
                    // at bounds of two nonempty paragraphs
                    // paragraph is nonempty because paragraphs cannot both be empty and touch a paragraph before/after
                    if backwards {
                        Some((self - 1, self))
                    } else {
                        Some((self, self + 1))
                    }
                } else if self == paragraph_before.end() {
                    // at end of paragraph before
                    // paragraph before is nonempty because Bounds::range_before does not consider the range (offset, offset) to be before offset
                    if backwards {
                        Some((self - 1, self))
                    } else {
                        Some((self, paragraph_after.start()))
                    }
                } else if self == paragraph_after.start() {
                    // at start of paragraph after
                    // paragraph after is nonempty because Bounds::range_after does not consider the range (offset, offset) to be after offset
                    if backwards {
                        Some((paragraph_before.end(), self))
                    } else {
                        Some((self, self + 1))
                    }
                } else {
                    // between paragraphs
                    Some((paragraph_before.end(), paragraph_after.start()))
                }
            }
        }
    }

    fn advance_bound(self, bound: Bound, backwards: bool, jump: bool, bounds: &Bounds) -> Self {
        if let Some(range) = self.range_bound(bound, backwards, jump, bounds) {
            if backwards {
                range.start()
            } else {
                range.end()
            }
        } else if bounds.paragraphs.len() > 0 {
            if backwards {
                bounds.paragraphs[0].start()
            } else {
                bounds.paragraphs[bounds.paragraphs.len() - 1].end()
            }
        } else {
            self
        }
    }

    /// Advances to a bound in a direction, stopping at the bound (e.g. cmd+left/right). If you're beyond the furthest
    /// bound, this snaps you into it, even if that moves you in the opposite direction.
    pub fn advance_to_bound(self, bound: Bound, backwards: bool, bounds: &Bounds) -> Self {
        self.advance_bound(bound, backwards, false, bounds)
    }

    /// Advances to a bound in a direction, jumping to the next bound if already at one (e.g. alt+left/right). If
    /// you're beyond the furthest bound, this snaps you into it, even if that moves you in the opposite direction.
    pub fn advance_to_next_bound(self, bound: Bound, backwards: bool, bounds: &Bounds) -> Self {
        self.advance_bound(bound, backwards, true, bounds)
    }
}

impl Editor {
    pub fn print_bounds(&self) {
        println!("words: {:?}", self.ranges_text(&self.bounds.words));
        println!("lines: {:?}", self.ranges_text(&self.bounds.lines));
        println!("paragraphs: {:?}", self.ranges_text(&self.bounds.paragraphs));
    }

    fn ranges_text(&self, ranges: &[(DocCharOffset, DocCharOffset)]) -> Vec<String> {
        ranges
            .iter()
            .map(|&range| self.buffer.current[range].to_string())
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod test {
    use crate::{input::canonical::Bound, offset_types::DocCharOffset};

    use super::Bounds;

    #[test]
    fn range_before_after_no_ranges() {
        let ranges = [];

        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(0)), None);
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(0)), None);
    }

    #[test]
    fn range_before_after_disjoint() {
        let ranges = [(1, 3), (5, 7), (9, 11)]
            .into_iter()
            .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
            .collect::<Vec<_>>();

        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(0)), None);
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(1)), None);
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(2)), Some(0));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(3)), Some(0));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(4)), Some(0));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(5)), Some(0));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(6)), Some(1));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(7)), Some(1));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(8)), Some(1));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(9)), Some(1));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(10)), Some(2));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(11)), Some(2));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(12)), Some(2));

        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(0)), Some(0));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(1)), Some(0));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(2)), Some(0));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(3)), Some(1));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(4)), Some(1));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(5)), Some(1));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(6)), Some(1));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(7)), Some(2));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(8)), Some(2));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(9)), Some(2));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(10)), Some(2));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(11)), None);
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(12)), None);
    }

    #[test]
    fn range_before_after_contiguous() {
        let ranges = [(1, 3), (3, 5), (5, 7)]
            .into_iter()
            .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
            .collect::<Vec<_>>();

        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(0)), None);
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(1)), None);
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(2)), Some(0));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(3)), Some(0));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(4)), Some(1));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(5)), Some(1));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(6)), Some(2));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(7)), Some(2));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(8)), Some(2));

        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(0)), Some(0));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(1)), Some(0));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(2)), Some(0));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(3)), Some(1));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(4)), Some(1));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(5)), Some(2));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(6)), Some(2));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(7)), None);
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(8)), None);
    }

    #[test]
    fn range_before_after_empty_ranges() {
        let ranges = [(1, 1), (3, 3), (5, 5)]
            .into_iter()
            .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
            .collect::<Vec<_>>();

        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(0)), None);
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(1)), None);
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(2)), Some(0));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(3)), Some(0));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(4)), Some(1));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(5)), Some(1));
        assert_eq!(Bounds::range_before(&ranges, DocCharOffset(6)), Some(2));

        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(0)), Some(0));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(1)), Some(1));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(2)), Some(1));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(3)), Some(2));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(4)), Some(2));
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(5)), None);
        assert_eq!(Bounds::range_after(&ranges, DocCharOffset(6)), None);
    }

    #[test]
    fn range_bound_no_ranges() {
        let bounds = Bounds::default();
        assert_eq!(DocCharOffset(0).range_bound(Bound::Word, false, false, &bounds), None);
        assert_eq!(DocCharOffset(0).range_bound(Bound::Word, true, false, &bounds), None);
        assert_eq!(DocCharOffset(0).range_bound(Bound::Word, false, true, &bounds), None);
        assert_eq!(DocCharOffset(0).range_bound(Bound::Word, true, true, &bounds), None);
    }

    #[test]
    fn range_bound_disjoint() {
        let words: Vec<(DocCharOffset, DocCharOffset)> = vec![(1, 3), (5, 7), (9, 11)]
            .into_iter()
            .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
            .collect();
        let bounds = Bounds { words, ..Default::default() };

        assert_eq!(
            DocCharOffset(0).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(1).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(2).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(9).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(10).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(11).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(12).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );

        assert_eq!(
            DocCharOffset(0).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(1).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(2).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(9).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(10).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(11).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(12).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );

        assert_eq!(
            DocCharOffset(0).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(1).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(2).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(9).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(10).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(DocCharOffset(11).range_bound(Bound::Word, false, true, &bounds), None);
        assert_eq!(DocCharOffset(12).range_bound(Bound::Word, false, true, &bounds), None);

        assert_eq!(DocCharOffset(0).range_bound(Bound::Word, true, true, &bounds), None);
        assert_eq!(DocCharOffset(1).range_bound(Bound::Word, true, true, &bounds), None);
        assert_eq!(
            DocCharOffset(2).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(9).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(10).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(11).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(12).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(9), DocCharOffset(11)))
        );
    }

    #[test]
    fn range_bound_contiguous() {
        let words: Vec<(DocCharOffset, DocCharOffset)> = vec![(1, 3), (3, 5), (5, 7)]
            .into_iter()
            .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
            .collect();
        let bounds = Bounds { words, ..Default::default() };

        assert_eq!(
            DocCharOffset(0).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(1).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(2).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );

        assert_eq!(
            DocCharOffset(0).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(1).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(2).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );

        assert_eq!(
            DocCharOffset(0).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(1).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(2).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound(Bound::Word, false, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(DocCharOffset(7).range_bound(Bound::Word, false, true, &bounds), None);
        assert_eq!(DocCharOffset(8).range_bound(Bound::Word, false, true, &bounds), None);

        assert_eq!(DocCharOffset(0).range_bound(Bound::Word, true, true, &bounds), None);
        assert_eq!(DocCharOffset(1).range_bound(Bound::Word, true, true, &bounds), None);
        assert_eq!(
            DocCharOffset(2).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound(Bound::Word, true, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(7)))
        );
    }

    #[test]
    fn advance_to_bound_no_ranges() {
        let bounds = Bounds { words: vec![], ..Default::default() };

        assert_eq!(
            DocCharOffset(0).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(0)
        );
        assert_eq!(DocCharOffset(0).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(0));
    }

    #[test]
    fn advance_to_bound_disjoint() {
        let words: Vec<(DocCharOffset, DocCharOffset)> = vec![(1, 3), (5, 7), (9, 11)]
            .into_iter()
            .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
            .collect();
        let bounds = Bounds { words, ..Default::default() };

        assert_eq!(
            DocCharOffset(0).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(9).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(10).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(11).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(12).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );

        assert_eq!(DocCharOffset(0).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(1).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(2).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(3).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(4).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(5).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
        assert_eq!(DocCharOffset(6).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
        assert_eq!(DocCharOffset(7).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
        assert_eq!(DocCharOffset(8).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
        assert_eq!(DocCharOffset(9).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(9));
        assert_eq!(
            DocCharOffset(10).advance_to_bound(Bound::Word, true, &bounds),
            DocCharOffset(9)
        );
        assert_eq!(
            DocCharOffset(11).advance_to_bound(Bound::Word, true, &bounds),
            DocCharOffset(9)
        );
        assert_eq!(
            DocCharOffset(12).advance_to_bound(Bound::Word, true, &bounds),
            DocCharOffset(9)
        );
    }

    #[test]
    fn advance_to_bound_contiguous() {
        let words: Vec<(DocCharOffset, DocCharOffset)> = vec![(1, 3), (3, 5), (5, 7)]
            .into_iter()
            .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
            .collect();
        let bounds = Bounds { words, ..Default::default() };

        assert_eq!(
            DocCharOffset(0).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );

        assert_eq!(DocCharOffset(0).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(1).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(2).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(3).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(4).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(3));
        assert_eq!(DocCharOffset(5).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(3));
        assert_eq!(DocCharOffset(6).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
        assert_eq!(DocCharOffset(7).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
        assert_eq!(DocCharOffset(8).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
    }

    #[test]
    fn advance_to_bound_empty_ranges() {
        let bounds = Bounds {
            words: vec![(1, 3), (5, 5), (7, 7), (9, 11)]
                .into_iter()
                .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
                .collect(),
            ..Default::default()
        };

        assert_eq!(
            DocCharOffset(0).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(9).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(10).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(11).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(12).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
    }

    #[test]
    fn advance_to_next_bound_no_ranges() {
        let bounds = Bounds::default();

        assert_eq!(
            DocCharOffset(0).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(0)
        );
        assert_eq!(
            DocCharOffset(0).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(0)
        );
    }

    #[test]
    fn advance_to_next_bound_disjoint() {
        let bounds = Bounds {
            words: vec![(1, 3), (5, 7), (9, 11)]
                .into_iter()
                .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
                .collect(),
            ..Default::default()
        };

        assert_eq!(
            DocCharOffset(0).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(9).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(10).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(11).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(12).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(12)
        );

        assert_eq!(
            DocCharOffset(0).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(0)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(9).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(10).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(9)
        );
        assert_eq!(
            DocCharOffset(11).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(9)
        );
        assert_eq!(
            DocCharOffset(12).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(9)
        );
    }

    #[test]
    fn advance_to_next_bound_contiguous() {
        let bounds = Bounds {
            words: vec![(1, 3), (3, 5), (5, 7)]
                .into_iter()
                .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
                .collect(),
            ..Default::default()
        };

        assert_eq!(
            DocCharOffset(0).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_next_bound(Bound::Word, false, &bounds),
            DocCharOffset(8)
        );

        assert_eq!(
            DocCharOffset(0).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(0)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_next_bound(Bound::Word, true, &bounds),
            DocCharOffset(5)
        );
    }

    #[test]
    fn range_bound_char_no_ranges() {
        let bounds = Bounds::default();

        assert_eq!(DocCharOffset(0).range_bound_char(false, false, &bounds), None);
        assert_eq!(DocCharOffset(0).range_bound_char(true, false, &bounds), None);
        assert_eq!(DocCharOffset(0).range_bound_char(false, true, &bounds), None);
        assert_eq!(DocCharOffset(0).range_bound_char(true, true, &bounds), None);
    }

    #[test]
    fn range_bound_char_disjoint() {
        let bounds = Bounds {
            paragraphs: vec![(1, 3), (5, 7), (9, 11)]
                .into_iter()
                .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
                .collect(),
            ..Default::default()
        };

        assert_eq!(
            DocCharOffset(0).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(1).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(2).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(2), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(6)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(6), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(9).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(10)))
        );
        assert_eq!(
            DocCharOffset(10).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(11).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(12).range_bound_char(false, false, &bounds),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );

        assert_eq!(
            DocCharOffset(0).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(1).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(2).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(2), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(5), DocCharOffset(6)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(6), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(9).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(10).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(9), DocCharOffset(10)))
        );
        assert_eq!(
            DocCharOffset(11).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(12).range_bound_char(true, false, &bounds),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );

        assert_eq!(
            DocCharOffset(0).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(1).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(2).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(2), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(6)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(6), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(9).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(9), DocCharOffset(10)))
        );
        assert_eq!(
            DocCharOffset(10).range_bound_char(false, true, &bounds),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
        assert_eq!(DocCharOffset(11).range_bound_char(false, true, &bounds), None);
        assert_eq!(DocCharOffset(12).range_bound_char(false, true, &bounds), None);

        assert_eq!(DocCharOffset(0).range_bound_char(true, true, &bounds), None);
        assert_eq!(DocCharOffset(1).range_bound_char(true, true, &bounds), None);
        assert_eq!(
            DocCharOffset(2).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(3).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(2), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(6).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(5), DocCharOffset(6)))
        );
        assert_eq!(
            DocCharOffset(7).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(6), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(9).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(10).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(9), DocCharOffset(10)))
        );
        assert_eq!(
            DocCharOffset(11).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(12).range_bound_char(true, true, &bounds),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
    }

    #[test]
    fn advance_to_next_bound_disjoint_char() {
        let bounds = Bounds {
            paragraphs: vec![(1, 3), (5, 7), (9, 11)]
                .into_iter()
                .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
                .collect(),
            ..Default::default()
        };

        assert_eq!(
            DocCharOffset(0).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(2)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(2)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(6)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(9)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(9)
        );
        assert_eq!(
            DocCharOffset(9).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(10)
        );
        assert_eq!(
            DocCharOffset(10).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(11).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(11)
        );
        assert_eq!(
            DocCharOffset(12).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(11)
        );

        assert_eq!(
            DocCharOffset(0).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(2)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(6)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(9).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(10).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(9)
        );
        assert_eq!(
            DocCharOffset(11).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(10)
        );
        assert_eq!(
            DocCharOffset(12).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(10)
        );
    }

    #[test]
    fn advance_to_next_bound_contiguous_char() {
        let bounds = Bounds {
            paragraphs: vec![(1, 3), (3, 5), (5, 7)]
                .into_iter()
                .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
                .collect(),
            ..Default::default()
        };

        assert_eq!(
            DocCharOffset(0).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(2)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(2)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(4)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(6)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_next_bound(Bound::Char, false, &bounds),
            DocCharOffset(7)
        );

        assert_eq!(
            DocCharOffset(0).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(1).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(2).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(1)
        );
        assert_eq!(
            DocCharOffset(3).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(2)
        );
        assert_eq!(
            DocCharOffset(4).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(3)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(4)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(6)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_next_bound(Bound::Char, true, &bounds),
            DocCharOffset(6)
        );
    }
}
