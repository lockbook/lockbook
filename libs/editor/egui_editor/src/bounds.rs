use crate::appearance::{Appearance, CaptureCondition};
use crate::ast::{Ast, AstTextRange, AstTextRangeType};
use crate::buffer::SubBuffer;
use crate::galleys::Galleys;
use crate::input::canonical::Bound;
use crate::input::cursor::Cursor;
use crate::offset_types::{DocByteOffset, DocCharOffset, RangeExt, RelByteOffset};
use crate::unicode_segs::UnicodeSegs;
use crate::Editor;
use egui::epaint::text::cursor::RCursor;
use linkify::LinkFinder;
use std::cmp::Ordering;
use std::collections::HashSet;
use unicode_segmentation::UnicodeSegmentation;

pub type AstTextRanges = Vec<AstTextRange>;
pub type Words = Vec<(DocCharOffset, DocCharOffset)>;
pub type Lines = Vec<(DocCharOffset, DocCharOffset)>;
pub type Paragraphs = Vec<(DocCharOffset, DocCharOffset)>;
pub type Text = Vec<(DocCharOffset, DocCharOffset)>;
pub type PlainTextLinks = Vec<(DocCharOffset, DocCharOffset)>;

/// Represents bounds of various text regions in the buffer. Region bounds are inclusive on both sides. Regions do not
/// overlap, have region.0 <= region.1, and are sorted. Character and doc regions are not stored explicitly but can be
/// inferred from the other regions.
#[derive(Debug, Default)]
pub struct Bounds {
    pub ast: AstTextRanges,

    pub words: Words,
    pub lines: Lines,
    pub paragraphs: Paragraphs,

    /// Text consists of all rendered text. Every valid cursor position is in some possibly-empty text range.
    pub text: Text,

    /// Plain text links are styled and clickable but aren't markdown links.
    pub links: PlainTextLinks,
}

pub fn calc_ast(ast: &Ast) -> AstTextRanges {
    ast.iter_text_ranges().collect()
}

pub fn calc_words(buffer: &SubBuffer, ast: &Ast, appearance: &Appearance) -> Words {
    let mut result = vec![];

    for text_range in ast.iter_text_ranges() {
        if text_range.range_type != AstTextRangeType::Text
            && appearance.markdown_capture(text_range.node(ast).node_type())
                == CaptureCondition::Always
        {
            // skip always-captured syntax sequences
            continue;
        } else if text_range.range_type != AstTextRangeType::Text
            && !text_range.node(ast).node_type().syntax_includes_text()
        {
            // syntax sequences for node types without text count as single words
            result.push(text_range.range);
        } else {
            // remaining text and syntax sequences (including link URLs etc) are split into words
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

    result
}

pub fn calc_lines(galleys: &Galleys, ast: &Ast, text: &Text) -> Lines {
    let mut result = vec![];
    let galleys = galleys;
    let mut text_range_iter = ast.iter_text_ranges();
    for (galley_idx, galley) in galleys.galleys.iter().enumerate() {
        for (row_idx, _) in galley.galley.rows.iter().enumerate() {
            let start_cursor = galley
                .galley
                .from_rcursor(RCursor { row: row_idx, column: 0 });
            let row_start =
                galleys.char_offset_by_galley_and_cursor(galley_idx, &start_cursor, text);
            let end_cursor = galley.galley.cursor_end_of_row(&start_cursor);
            let row_end = galleys.char_offset_by_galley_and_cursor(galley_idx, &end_cursor, text);

            let mut range = (row_start, row_end);

            // rows in galley head/tail are excluded
            if row_end < galley.text_range().start() {
                continue;
            }
            if row_start > galley.text_range().end() {
                break;
            }

            // if the range bounds are in the middle of a syntax sequence, expand the range to include the whole sequence
            // this supports selecting a line that starts or ends with a syntax sequence that's captured until the selection happens
            for text_range in text_range_iter.by_ref() {
                if text_range.range.start() > range.end() {
                    break;
                }
                if text_range.range_type == AstTextRangeType::Text {
                    continue;
                }
                if text_range.range.contains_inclusive(range.0) {
                    range.0 = text_range.range.0;
                }
                if text_range.range.contains_inclusive(range.1) {
                    range.1 = text_range.range.1;
                    break;
                }
            }

            // bound row start and row end by the galley bounds
            let (min, max) = galley.text_range();
            range.0 = range.0.max(min).min(max);
            range.1 = range.1.max(min).min(max);

            result.push(range)
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

pub fn calc_text(ast: &Ast, appearance: &Appearance, segs: &UnicodeSegs, cursor: Cursor) -> Text {
    let mut result = vec![];
    let mut last_range_pushed = false;
    for text_range in ast.iter_text_ranges() {
        let captured = match appearance.markdown_capture(text_range.node(ast).node_type()) {
            CaptureCondition::Always => true,
            CaptureCondition::NoCursor => !text_range.intersects_selection(ast, cursor),
            CaptureCondition::Never => false,
        };

        let this_range_pushed = if text_range.range_type == AstTextRangeType::Text || !captured {
            // text range or uncaptured syntax range
            result.push(text_range.range);
            true
        } else {
            false
        };

        if !this_range_pushed && !last_range_pushed {
            // empty range between captured ranges
            result.push((text_range.range.0, text_range.range.0));
        }
        last_range_pushed = this_range_pushed;
    }

    if !last_range_pushed {
        // empty range at end of doc
        result.push((segs.last_cursor_position(), segs.last_cursor_position()));
    }
    if result.is_empty() {
        result = vec![(0.into(), 0.into())];
    }

    result
}

pub fn calc_links(buffer: &SubBuffer, text: &Text) -> PlainTextLinks {
    let finder = {
        let mut this = LinkFinder::new();
        this.kinds(&[linkify::LinkKind::Url])
            .url_must_have_scheme(false)
            .url_can_be_iri(false); // ignore links with international characters for phishing prevention
        this
    };

    let mut result = vec![];
    for &text_range in text {
        for span in finder.spans(&buffer[text_range]) {
            if span.kind().is_some() {
                result.push((text_range.0 + span.start(), text_range.0 + span.end()));
            }
        }
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

pub enum BoundCase {
    // |
    NoRanges,
    // |xx yy
    AtFirstRangeStart {
        first_range: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
    // xx yy|
    AtLastRangeEnd {
        last_range: (DocCharOffset, DocCharOffset),
        range_before: (DocCharOffset, DocCharOffset),
    },
    // x|x yy
    InsideRange {
        range: (DocCharOffset, DocCharOffset),
    },
    /*
     *  xx
     *  |
     *  yy
     */
    AtEmptyRange {
        range: (DocCharOffset, DocCharOffset),
        range_before: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
    // xx|yy
    // both ranges nonempty
    AtRangesBound {
        range_before: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
    // xx| yy
    // range before is nonempty
    AtEndOfRangeBefore {
        range_before: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
    // xx |yy
    // range after is nonempty
    AtStartOfRangeAfter {
        range_before: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
    // xx | yy
    BetweenRanges {
        range_before: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
}

impl DocCharOffset {
    /// Returns the range in the direction of `backwards` from offset `self`. If `jump` is true, `self` will not be at
    /// the boundary of the result in the direction of `backwards` (e.g. alt+left/right), otherwise it will be (e.g.
    /// cmd+left/right). For instance, if `jump` is true, advancing beyond the first or last range in the doc will
    /// return None, otherwise it will return the first or last range in the doc.
    pub fn range_bound(
        self, bound: Bound, backwards: bool, jump: bool, bounds: &Bounds,
    ) -> Option<(Self, Self)> {
        let ranges = match bound {
            Bound::Char => {
                return self.char_bound(backwards, jump, &bounds.text);
            }
            Bound::Word => &bounds.words,
            Bound::Line => &bounds.lines,
            Bound::Paragraph => &bounds.paragraphs,
            Bound::Doc => {
                return Some((
                    bounds
                        .text
                        .first()
                        .map(|(start, _)| *start)
                        .unwrap_or(DocCharOffset(0)),
                    bounds
                        .text
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
            match self.bound_case(ranges) {
                BoundCase::NoRanges => None,
                BoundCase::AtFirstRangeStart { first_range, .. } => {
                    if backwards && jump {
                        // jump backwards off the edge from the start of the first range
                        None
                    } else {
                        Some(first_range)
                    }
                }
                BoundCase::AtLastRangeEnd { last_range, .. } => {
                    if !backwards && jump {
                        // jump forwards off the edge from the end of the last range
                        None
                    } else {
                        Some(last_range)
                    }
                }
                BoundCase::InsideRange { range } => Some(range),
                BoundCase::AtEmptyRange { range, .. } => Some(range),
                BoundCase::AtRangesBound { range_before, range_after } => {
                    if backwards {
                        Some(range_before)
                    } else {
                        Some(range_after)
                    }
                }
                BoundCase::AtEndOfRangeBefore { range_before, .. } => Some(range_before),
                BoundCase::AtStartOfRangeAfter { range_after, .. } => Some(range_after),
                BoundCase::BetweenRanges { range_before, range_after } => {
                    if backwards {
                        Some(range_before)
                    } else {
                        Some(range_after)
                    }
                }
            }
        }
    }

    /// Returns the range in the direction of `backwards` from offset `self` representing a single character. If a
    /// range is returned, it's either a single character in a paragraph or a nonempty range between paragraphs. If
    /// `jump` is true, advancing beyond the first or last character in the doc will return None, otherwise it will
    /// return the first or last character in the doc.
    fn char_bound(self, backwards: bool, jump: bool, text: &Text) -> Option<(Self, Self)> {
        match self.bound_case(text) {
            BoundCase::NoRanges => None,
            BoundCase::AtFirstRangeStart { first_range, range_after } => {
                if backwards && jump {
                    // jump backwards off the edge from the start of the first paragraph
                    None
                } else if first_range.is_empty() {
                    // nonempty range between paragraphs
                    // paragraph after is not first_paragraph because Bounds::range_after does not consider the range (offset, offset) to be after offset
                    // range is nonempty because paragraphs cannot both be empty and touch a paragraph before/after
                    Some((first_range.start(), range_after.start()))
                } else {
                    // first character of the first paragraph
                    Some((first_range.start(), first_range.start() + 1))
                }
            }
            BoundCase::AtLastRangeEnd { last_range, range_before } => {
                if !backwards && jump {
                    // jump forwards off the edge from the end of the last paragraph
                    None
                } else if last_range.is_empty() {
                    // nonempty range between paragraphs
                    // paragraph before is not last_paragraph because Bounds::range_before does not consider the range (offset, offset) to be before offset
                    // range is nonempty because paragraphs cannot both be empty and touch a paragraph before/after
                    Some((range_before.end(), last_range.end()))
                } else {
                    // last character of the last paragraph
                    Some((last_range.end() - 1, last_range.end()))
                }
            }
            BoundCase::InsideRange { .. } => {
                if backwards {
                    Some((self - 1, self))
                } else {
                    Some((self, self + 1))
                }
            }
            BoundCase::AtEmptyRange { range: _, range_before, range_after } => {
                if backwards {
                    Some((range_before.end(), self))
                } else {
                    Some((self, range_after.start()))
                }
            }
            BoundCase::AtRangesBound { .. } => {
                if backwards {
                    Some((self - 1, self))
                } else {
                    Some((self, self + 1))
                }
            }
            BoundCase::AtEndOfRangeBefore { range_after, .. } => {
                if backwards {
                    Some((self - 1, self))
                } else {
                    Some((self, range_after.start()))
                }
            }
            BoundCase::AtStartOfRangeAfter { range_before, .. } => {
                if backwards {
                    Some((range_before.end(), self))
                } else {
                    Some((self, self + 1))
                }
            }
            BoundCase::BetweenRanges { range_before, range_after } => {
                Some((range_before.end(), range_after.start()))
            }
        }
    }

    pub fn bound_case(self, ranges: &[(DocCharOffset, DocCharOffset)]) -> BoundCase {
        let range_before = Bounds::range_before(ranges, self);
        let range_after = Bounds::range_after(ranges, self);
        match (range_before, range_after) {
            (None, None) => BoundCase::NoRanges,
            // before or at the start of the first range
            (None, Some(range_after)) => {
                let first_range = ranges[0];
                let range_after = ranges[range_after];
                if self < first_range.start() {
                    // a cursor before the first range is considered at the start of the first range
                    first_range.start().bound_case(ranges)
                } else {
                    // self == first_range.start() because otherwise we'd have a range before
                    BoundCase::AtFirstRangeStart { first_range, range_after }
                }
            }
            // after or at the end of the last range
            (Some(range_before), None) => {
                let last_range = ranges[ranges.len() - 1];
                let range_before = ranges[range_before];
                if self > last_range.end() {
                    // a cursor after the last range is considered at the end of the last range
                    last_range.end().bound_case(ranges)
                } else {
                    // self == last_range.end() because otherwise we'd have a range after
                    BoundCase::AtLastRangeEnd { last_range, range_before }
                }
            }
            (Some(range_before), Some(range_after)) if range_before == range_after => {
                BoundCase::InsideRange { range: ranges[range_before] }
            }
            (Some(range_before_idx), Some(range_after_idx)) => {
                let range_before = ranges[range_before_idx];
                let range_after = ranges[range_after_idx];
                if range_before_idx + 1 != range_after_idx {
                    BoundCase::AtEmptyRange { range: (self, self), range_before, range_after }
                } else if range_before.end() == range_after.start() {
                    BoundCase::AtRangesBound { range_before, range_after }
                } else if self == range_before.end() {
                    BoundCase::AtEndOfRangeBefore { range_before, range_after }
                } else if self == range_after.start() {
                    BoundCase::AtStartOfRangeAfter { range_before, range_after }
                } else {
                    BoundCase::BetweenRanges { range_before, range_after }
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
        } else if !bounds.text.is_empty() {
            if backwards {
                bounds.text[0].start()
            } else {
                bounds.text[bounds.text.len() - 1].end()
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

pub trait RangesExt {
    /// Efficiently finds the sorted, possibly empty set of ranges that contain `offset`
    fn find(&self, offset: DocCharOffset, start_inclusive: bool, end_inclusive: bool)
        -> Vec<usize>;
}

impl<Range: RangeExt<DocCharOffset>> RangesExt for Vec<Range> {
    fn find(
        &self, offset: DocCharOffset, start_inclusive: bool, end_inclusive: bool,
    ) -> Vec<usize> {
        let mut result = Vec::new();
        if let Ok(idx) = self.binary_search_by(|range| {
            if offset < range.start() {
                Ordering::Less
            } else if offset == range.start() {
                if start_inclusive {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            } else if offset > range.start() && offset > range.end() {
                Ordering::Equal
            } else if offset == range.end() {
                if end_inclusive {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            } else if offset > range.end() {
                Ordering::Greater
            } else {
                unreachable!()
            }
        }) {
            let mut start_idx = idx;
            while idx > 0 && self[idx - 1].contains(offset, start_inclusive, end_inclusive) {
                start_idx -= 1;
                result.push(start_idx);
            }
            result.reverse();

            result.push(idx);

            let mut end_idx = idx;
            while end_idx < self.len() - 1
                && self[end_idx + 1].contains(offset, start_inclusive, end_inclusive)
            {
                end_idx += 1;
                result.push(end_idx);
            }
        }
        result
    }
}

impl Editor {
    pub fn print_bounds(&self) {
        println!("words: {:?}", self.ranges_text(&self.bounds.words));
        println!("lines: {:?}", self.ranges_text(&self.bounds.lines));
        println!("paragraphs: {:?}", self.ranges_text(&self.bounds.paragraphs));
        println!("text: {:?}", self.ranges_text(&self.bounds.text));
        println!("links: {:?}", self.ranges_text(&self.bounds.links));
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
    fn char_bound_no_ranges() {
        let bounds = Bounds::default();

        assert_eq!(DocCharOffset(0).char_bound(false, false, &bounds.text), None);
        assert_eq!(DocCharOffset(0).char_bound(true, false, &bounds.text), None);
        assert_eq!(DocCharOffset(0).char_bound(false, true, &bounds.text), None);
        assert_eq!(DocCharOffset(0).char_bound(true, true, &bounds.text), None);
    }

    #[test]
    fn char_bound_disjoint() {
        let bounds = Bounds {
            text: vec![(1, 3), (5, 7), (9, 11)]
                .into_iter()
                .map(|(start, end)| (DocCharOffset(start), DocCharOffset(end)))
                .collect(),
            ..Default::default()
        };

        assert_eq!(
            DocCharOffset(0).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(1).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(2).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(2), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(4).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(5), DocCharOffset(6)))
        );
        assert_eq!(
            DocCharOffset(6).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(6), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(8).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(9).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(9), DocCharOffset(10)))
        );
        assert_eq!(
            DocCharOffset(10).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(11).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(12).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );

        assert_eq!(
            DocCharOffset(0).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(1).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(2).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(3).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(2), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(6).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(5), DocCharOffset(6)))
        );
        assert_eq!(
            DocCharOffset(7).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(6), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(9).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(10).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(9), DocCharOffset(10)))
        );
        assert_eq!(
            DocCharOffset(11).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(12).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );

        assert_eq!(
            DocCharOffset(0).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(1).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(2).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(2), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(4).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(5), DocCharOffset(6)))
        );
        assert_eq!(
            DocCharOffset(6).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(6), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(8).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(9).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(9), DocCharOffset(10)))
        );
        assert_eq!(
            DocCharOffset(10).char_bound(false, true, &bounds.text),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
        assert_eq!(DocCharOffset(11).char_bound(false, true, &bounds.text), None);
        assert_eq!(DocCharOffset(12).char_bound(false, true, &bounds.text), None);

        assert_eq!(DocCharOffset(0).char_bound(true, true, &bounds.text), None);
        assert_eq!(DocCharOffset(1).char_bound(true, true, &bounds.text), None);
        assert_eq!(
            DocCharOffset(2).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(3).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(2), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(6).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(5), DocCharOffset(6)))
        );
        assert_eq!(
            DocCharOffset(7).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(6), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(9).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(10).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(9), DocCharOffset(10)))
        );
        assert_eq!(
            DocCharOffset(11).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
        assert_eq!(
            DocCharOffset(12).char_bound(true, true, &bounds.text),
            Some((DocCharOffset(10), DocCharOffset(11)))
        );
    }

    #[test]
    fn advance_to_next_bound_disjoint_char() {
        let bounds = Bounds {
            text: vec![(1, 3), (5, 7), (9, 11)]
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
            text: vec![(1, 3), (3, 5), (5, 7)]
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
