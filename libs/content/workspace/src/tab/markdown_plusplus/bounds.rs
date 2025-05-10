// use crate::tab::markdown_plusplus::appearance::{Appearance, CaptureCondition};
// use crate::tab::markdown_plusplus::ast::{Ast, AstTextRange, AstTextRangeType};
// use crate::tab::markdown_plusplus::galleys::Galleys;
// use crate::tab::markdown_plusplus::input::capture::CaptureState;
// use crate::tab::markdown_plusplus::input::Bound;
// use crate::tab::markdown_plusplus::style::{BlockNodeType, InlineNodeType, MarkdownNodeType};
use crate::tab::markdown_plusplus::MarkdownPlusPlus;
use comrak::nodes::{LineColumn, Sourcepos};
use lb_rs::model::text::offset_types::{DocByteOffset, DocCharOffset, RangeExt, RelByteOffset};
use std::cmp::Ordering;
use std::ops::Sub;

use super::input::Bound;

pub type SourceLines = Vec<(DocCharOffset, DocCharOffset)>;
pub type Words = Vec<(DocCharOffset, DocCharOffset)>;
pub type Lines = Vec<(DocCharOffset, DocCharOffset)>;
pub type Paragraphs = Vec<(DocCharOffset, DocCharOffset)>;
pub type Text = Vec<(DocCharOffset, DocCharOffset)>;

/// Represents bounds of various text regions in the buffer. Region bounds are (inclusive, exclusive). Regions do not
/// overlap, have region.0 <= region.1, and are sorted ascending.
#[derive(Debug, Default)]
pub struct Bounds {
    /// Source lines are separated by newline characters and include all characters in the document except the newline
    /// characters. These accelerate translating AST source positions to character offsets and back and are computed
    /// early in the frame using few dependencies.
    /// * Documents have at least one source line.
    /// * Source lines can be empty.
    /// * Source lines cannot touch.
    pub source_lines: SourceLines,

    /// Words are separated by UAX#29 (Unicode Standard Annex #29) word boundaries and do not contain whitespace. Some
    /// punctuation marks count as words. These are used for word-based cursor movement and selection e.g. double click
    /// and alt+left/right.
    /// * Documents may have no words.
    /// * Words cannot be empty.
    /// * Words can touch.
    pub words: Words,

    /// Lines are separated by newline characters or by line wrap. These are used for line-based cursor movement and
    /// selection e.g. cmd+left/right, home/end, and cmd+up/down.
    /// * Documents have at least one line.
    /// * Lines can be empty.
    /// * Lines can touch.
    pub lines: Lines,

    /// Paragraphs are separated by newline characters. All inlines are contained within a paragraph. This definition
    /// includes table cells, code block info strings, and everywhere else that shows editable text. Paragraphs also
    /// contain hidden characters like captured syntax that should be copied with selected text.
    /// * Documents have at least one paragraph.
    /// * Paragraphs can be empty.
    /// * Paragraphs cannot touch.
    pub paragraphs: Paragraphs,

    /// Text consists of all rendered text separated by captured syntax ranges. Every valid cursor position is in some text
    /// range.
    /// * Documents have at least one text range.
    /// * Text ranges can be empty.
    /// * Text ranges can touch.
    pub text: Text,
}

impl MarkdownPlusPlus {
    pub fn calc_source_lines(&mut self) {
        self.bounds.source_lines.clear();

        let doc = (0.into(), self.last_cursor_position());
        self.bounds.source_lines = self.range_lines(doc);
    }

    /// Translates a comrak::LineColumn into an lb_rs::DocCharOffset. Note that comrak's text ranges, represented using
    /// comrak::Sourcepos, are inclusive/inclusive so just translating the start and end using this function is
    /// incorrect - use [`sourcepos_to_range`] instead.
    pub fn line_column_to_offset(&self, line_column: LineColumn) -> DocCharOffset {
        // convert cardinal to ordinal
        let line_column = if line_column.column == 0 {
            LineColumn { line: line_column.line.saturating_sub(2), column: line_column.column }
        } else {
            LineColumn { line: line_column.line.saturating_sub(1), column: line_column.column - 1 }
        };

        let line: (DocCharOffset, DocCharOffset) = *self
            .bounds
            .source_lines
            .get(line_column.line)
            .expect("source line should be in bounds");
        let line_start_byte = self.offset_to_byte(line.start());
        let line_column_byte = line_start_byte + line_column.column;
        self.offset_to_char(line_column_byte)
    }

    pub fn offset_to_line_column(&self, offset: DocCharOffset) -> LineColumn {
        let line_idx = self
            .bounds
            .source_lines
            .find_containing(offset, true, true)
            .start();
        let line = self.bounds.source_lines[line_idx];
        let line_start_byte = self.offset_to_byte(line.start());
        let line_column_byte = self.offset_to_byte(offset) - line_start_byte;

        // convert ordinal to cardinal
        LineColumn { line: line_idx + 1, column: line_column_byte.0 + 1 }
    }

    pub fn sourcepos_to_range(&self, sourcepos: Sourcepos) -> (DocCharOffset, DocCharOffset) {
        let start = self.line_column_to_offset(sourcepos.start);
        let end = self.line_column_to_offset(sourcepos.end);

        // convert (inc, inc) pair to (inc, exc) pair
        (start, end + 1)
    }

    pub fn range_to_sourcepos(&self, range: (DocCharOffset, DocCharOffset)) -> Sourcepos {
        // convert (inc, exc) pair to (inc, inc) pair
        let range = (range.0, range.1 - 1);

        let start = self.offset_to_line_column(range.0);
        let end = self.offset_to_line_column(range.1);

        Sourcepos { start, end }
    }
}

impl Bounds {
    /// Returns the last range with start < char_offset <= end, or None if there's no such range.
    // todo: binary search
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

    /// Returns the first range with start <= char_offset < end, or None if there's no such range.
    // todo: binary search
    fn range_after(
        ranges: &[(DocCharOffset, DocCharOffset)], char_offset: DocCharOffset,
    ) -> Option<usize> {
        ranges
            .iter()
            .enumerate()
            .find(|(_, &range)| char_offset < range.end())
            .map(|(idx, _)| idx)
    }

    // pub fn ast_ranges(&self) -> Vec<(DocCharOffset, DocCharOffset)> {
    //     self.ast.iter().map(|text_range| text_range.range).collect()
    // }
}

#[derive(Debug)]
pub enum BoundCase {
    /// There are no ranges to contextualize the position.
    ///
    /// |
    NoRanges,
    /// The position is at the start of the first range. This may or may not be the start of the document e.g. the
    /// first word in the document may be preceded by whitespace. Positions in the empty space before the first range
    /// are also described by this variant.
    ///
    /// |(range)
    AtFirstRangeStart {
        first_range: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
    /// The position is at the end of the last range. This may or may not be the end of the document e.g. the last word
    /// in the document may be followed by whitespace.
    ///
    /// (range)|
    AtLastRangeEnd {
        last_range: (DocCharOffset, DocCharOffset),
        range_before: (DocCharOffset, DocCharOffset),
    },
    /// The position is inside a range and not at its start or end. The range must have length at least 2.
    ///
    /// (ra|nge)
    InsideRange { range: (DocCharOffset, DocCharOffset) },
    /// The position is at the start/end of an empty range.
    ///
    /// (|)
    AtEmptyRange {
        range: (DocCharOffset, DocCharOffset),
        range_before: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
    /// The position is between two ranges, both at the end of the range before and the start of the range after.
    ///
    /// (range1)|(range2)
    AtSharedBoundOfTouchingNonemptyRanges {
        range_before: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
    /// The position is at the end of a nonempty range with space between it and the range after. There is a range
    /// after: otherwise, the variant would be AtLastRangeEnd.
    ///
    /// (range1)| (range2)
    AtEndOfNonemptyRange {
        range_before: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
    /// The position is at the start of a nonempty range with space between it and the range before. There is a range
    /// before: otherwise, the variant would be AtFirstRangeStart.
    ///
    /// (range1) |(range2)
    AtStartOfNonemptyRange {
        range_before: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
    /// The position is between two ranges without being at the start/end of either range. It is inside the space
    /// between ranges.
    ///
    /// (range1) | (range2)
    BetweenRanges {
        range_before: (DocCharOffset, DocCharOffset),
        range_after: (DocCharOffset, DocCharOffset),
    },
}

pub trait BoundExt {
    fn range_bound(
        self, bound: Bound, backwards: bool, jump: bool, bounds: &Bounds,
    ) -> Option<(Self, Self)>
    where
        Self: Sized;
    fn char_bound(self, backwards: bool, jump: bool, text: &Text) -> Option<(Self, Self)>
    where
        Self: Sized;
    fn bound_case(self, ranges: &[(DocCharOffset, DocCharOffset)]) -> BoundCase;
    fn advance_bound(self, bound: Bound, backwards: bool, jump: bool, bounds: &Bounds) -> Self;
    fn advance_to_bound(self, bound: Bound, backwards: bool, bounds: &Bounds) -> Self;
    fn advance_to_next_bound(self, bound: Bound, backwards: bool, bounds: &Bounds) -> Self;
}

impl BoundExt for DocCharOffset {
    /// Returns the range in the direction of `backwards` from offset `self`. `jump` is used to control behavior when
    /// `self` is at a boundary in the direction of `backwards`. When `backwards` and `jump` are false and `self` is at
    /// the end of a range, returns that range. When `backwards` is `false` but `jump` is true and `self` is at the end
    /// of a range, returns the next range. If `jump` is true, advancing beyond the first or last character in the doc
    /// will return None, otherwise it will return the first or last range in the doc.
    ///
    /// For example, `jump` would be set to `true` when implementing alt+left/right behavior, which should always move
    /// the cursor to the next word, but set to `false` when implementing cmd+left/right behavior, which should not
    /// move the cursor if it is already at the line bound in the same direction.
    fn range_bound(
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
                BoundCase::AtSharedBoundOfTouchingNonemptyRanges { range_before, range_after } => {
                    if backwards {
                        Some(range_after)
                    } else {
                        Some(range_before)
                    }
                }
                BoundCase::AtEndOfNonemptyRange { range_before, .. } => Some(range_before),
                BoundCase::AtStartOfNonemptyRange { range_after, .. } => Some(range_after),
                BoundCase::BetweenRanges { .. } => None,
            }
        }
    }

    /// Returns the range in the direction of `backwards` from offset `self` representing a single character for the
    /// purposes of cursor navigation. Spaces between ranges of rendered text, including markdown syntax sequences
    /// replaced with bullets or hidden entirely, are considered single characters so that the cursor navigates over
    /// them in one keystroke.
    ///
    /// If a range is returned, it's either a single unicode character or a nonempty range between rendered text. If
    /// `jump` is true, advancing beyond the first or last character in the doc will return None, otherwise it will
    /// return the first or last character in the doc.
    fn char_bound(self, backwards: bool, jump: bool, text: &Text) -> Option<(Self, Self)> {
        match self.bound_case(text) {
            BoundCase::NoRanges => None, // never happens because we always have at least one text range
            BoundCase::AtFirstRangeStart { first_range, range_after } => {
                if backwards && jump {
                    // jump backwards off the edge from the start of the first paragraph
                    None
                } else if first_range.is_empty() {
                    // nonempty range between paragraphs
                    // paragraph after is not first_paragraph because Bounds::range_after does not consider the range (offset, offset) to be after offset
                    // range is nonempty because paragraphs cannot both be empty and touch a paragraph before/after
                    Some((first_range.end(), range_after.start()))
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
                    Some((range_before.end(), last_range.start()))
                } else {
                    // last character of the last paragraph
                    Some((last_range.end() - 1, last_range.end()))
                }
            }
            BoundCase::InsideRange { .. } => {
                if backwards ^ !jump {
                    Some((self - 1, self))
                } else {
                    Some((self, self + 1))
                }
            }
            BoundCase::AtEmptyRange { range: _, range_before, range_after } => {
                if backwards ^ !jump {
                    if self == range_before.end() {
                        // assumes we don't have multiple empty ranges on top of
                        // each other, which falls under the broader assumption
                        // that we have no duplicate ranges...
                        Some((range_before.end() - 1, range_before.end()))
                    } else {
                        Some((range_before.end(), self))
                    }
                } else {
                    #[allow(clippy::collapsible_else_if)]
                    if self == range_after.start() {
                        // ...same assumption here
                        Some((range_after.start(), range_after.start() + 1))
                    } else {
                        Some((self, range_after.start()))
                    }
                }
            }
            BoundCase::AtSharedBoundOfTouchingNonemptyRanges { .. } => {
                if backwards ^ !jump {
                    Some((self - 1, self))
                } else {
                    Some((self, self + 1))
                }
            }
            BoundCase::AtEndOfNonemptyRange { range_after, .. } => {
                if backwards ^ !jump {
                    Some((self - 1, self))
                } else {
                    Some((self, range_after.start()))
                }
            }
            BoundCase::AtStartOfNonemptyRange { range_before, .. } => {
                if backwards ^ !jump {
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

    // todo: broken when first/last range are empty (did those ranges need to be differentiated anyway?)
    fn bound_case(self, ranges: &[(DocCharOffset, DocCharOffset)]) -> BoundCase {
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
                    BoundCase::AtSharedBoundOfTouchingNonemptyRanges { range_before, range_after }
                } else if self == range_before.end() {
                    BoundCase::AtEndOfNonemptyRange { range_before, range_after }
                } else if self == range_after.start() {
                    BoundCase::AtStartOfNonemptyRange { range_before, range_after }
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
    /// bound, this snaps you into it, even if that moves you in the opposite direction. If you're not in a bound e.g.
    /// jumping to end of word while not in a word, this does nothing.
    fn advance_to_bound(self, bound: Bound, backwards: bool, bounds: &Bounds) -> Self {
        self.advance_bound(bound, backwards, false, bounds)
    }

    /// Advances to a bound in a direction, jumping to the next bound if already at one (e.g. alt+left/right). If
    /// you're beyond the furthest bound, this snaps you into it, even if that moves you in the opposite direction.
    fn advance_to_next_bound(self, bound: Bound, backwards: bool, bounds: &Bounds) -> Self {
        self.advance_bound(bound, backwards, true, bounds)
    }
}

pub trait RangesExt {
    type Element: Copy + Sub<Self::Element>;

    /// Efficiently finds the possibly empty (inclusive, exclusive) range of ranges that contain `offset`.
    /// When no ranges contain `offset`, result.start() == result.end() == the index of the first range after `offset`.
    fn find_containing(
        &self, offset: Self::Element, start_inclusive: bool, end_inclusive: bool,
    ) -> (usize, usize);

    /// Efficiently finds the possibly empty (inclusive, exclusive) range of ranges that are contained by `range`.
    /// When no ranges are contained by `range`, result.start() == result.end() == the index of the first range after `range`.
    fn find_contained(
        &self, range: (Self::Element, Self::Element), start_inclusive: bool, end_inclusive: bool,
    ) -> (usize, usize);

    /// Efficiently finds the possibly empty (inclusive, exclusive) range of ranges that intersect `range`.
    /// When no ranges intersect `range`, result.start() == result.end() == the index of the first range after `range`.
    fn find_intersecting(
        &self, range: (Self::Element, Self::Element), allow_empty: bool,
    ) -> (usize, usize);
}

impl<Range: RangeExt> RangesExt for Vec<Range>
where
    Range::Element: Copy + Sub<Range::Element> + Ord,
{
    type Element = Range::Element;

    fn find_containing(
        &self, offset: Range::Element, start_inclusive: bool, end_inclusive: bool,
    ) -> (usize, usize) {
        match self.binary_search_by(|range| {
            if offset < range.start() {
                Ordering::Greater
            } else if offset == range.start() {
                if offset == range.end() && !end_inclusive {
                    Ordering::Less
                } else if start_inclusive {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            } else if offset > range.start() && offset < range.end() {
                Ordering::Equal
            } else if offset == range.end() {
                if end_inclusive {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            } else if offset > range.end() {
                Ordering::Less
            } else {
                unreachable!()
            }
        }) {
            Ok(idx) => {
                let mut start = idx;
                while start > 0 && self[start - 1].contains(offset, start_inclusive, end_inclusive)
                {
                    start -= 1;
                }

                let mut end = idx;
                while end < self.len() && self[end].contains(offset, start_inclusive, end_inclusive)
                {
                    end += 1;
                }

                (start, end)
            }
            Err(idx) => (idx, idx),
        }
    }

    fn find_contained(
        &self, range: (Range::Element, Range::Element), start_inclusive: bool, end_inclusive: bool,
    ) -> (usize, usize) {
        let (mut start, mut end) = self.find_intersecting(range, true);
        while start < end
            && !range.contains_range(
                &(self[start].start(), self[start].end()),
                start_inclusive,
                end_inclusive,
            )
        {
            start += 1;
        }
        while end > start
            && !range.contains_range(
                &(self[end - 1].start(), self[end - 1].end()),
                start_inclusive,
                end_inclusive,
            )
        {
            end -= 1;
        }
        (start, end)
    }

    fn find_intersecting(
        &self, range: (Range::Element, Range::Element), allow_empty: bool,
    ) -> (usize, usize) {
        let (start_start, _) = self.find_containing(range.start(), false, allow_empty);
        let (_, end_end) = self.find_containing(range.end(), allow_empty, false);
        (start_start, end_end)
    }
}

pub fn join<const N: usize>(ranges: [&[(DocCharOffset, DocCharOffset)]; N]) -> RangeJoinIter<N> {
    let mut result = RangeJoinIter {
        ranges,
        in_range: [false; N],
        current: [None; N],
        current_end: Some(0.into()),
    };
    for (idx, range) in ranges.iter().enumerate() {
        if !range.is_empty() {
            result.current[idx] = Some(0);
        }
    }
    result
}

pub struct RangeJoinIter<'r, const N: usize> {
    ranges: [&'r [(DocCharOffset, DocCharOffset)]; N],
    in_range: [bool; N],
    current: [Option<usize>; N],
    current_end: Option<DocCharOffset>,
}

impl<'r, const N: usize> Iterator for RangeJoinIter<'r, N> {
    type Item = ([Option<usize>; N], (DocCharOffset, DocCharOffset));

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current_end) = self.current_end {
            // advance all ranges that end at next end
            for idx in 0..self.in_range.len() {
                let in_range = self.in_range[idx];

                // range set must not be out of ranges
                if let Some(current) = self.current[idx] {
                    let range = self.ranges[idx][current];

                    // must be at end of current range
                    if (in_range && current_end == range.end())
                        || (!in_range && current_end == range.start())
                    {
                        if !in_range {
                            // advance to the range after the current between-ranges range
                            self.in_range[idx] = true;
                        } else {
                            // advance to the next range, if any
                            if current < self.ranges[idx].len() - 1 {
                                self.current[idx] = Some(current + 1);

                                // if the next range starts after next_end, we're between ranges
                                if self.ranges[idx][current + 1].start() > current_end {
                                    self.in_range[idx] = false;
                                }
                            } else {
                                self.current[idx] = None;
                            }
                        }
                    }
                }
            }

            // exclude between-ranges ranges from result
            let idx_result = {
                let mut this = self.current;
                for (idx, &in_range) in self.in_range.iter().enumerate() {
                    if !in_range {
                        this[idx] = None;
                    }
                }
                this
            };

            // determine the next end of a range
            let mut next_end: Option<DocCharOffset> = None;
            for (idx, &in_range) in self.in_range.iter().enumerate() {
                let next_range = if let Some(next) = self.current[idx] {
                    self.ranges[idx][next]
                } else {
                    // when we're beyond the last range in a set of ranges, we no longer consider that set's next range
                    continue;
                };

                let end = if in_range {
                    next_range.end()
                } else {
                    // if we're not in a range, we're between ranges and next stores the next one
                    // the start of the next range is the end of the between-ranges range
                    next_range.start()
                };

                next_end =
                    if let Some(next_end) = next_end { Some(next_end.min(end)) } else { Some(end) };
            }

            // if there's no next end of a range, we're beyond the last range in all sets of ranges, so we're done
            let next_end = if let Some(next_end) = next_end {
                self.current_end = Some(next_end);
                next_end
            } else {
                return None;
            };

            Some((idx_result, (current_end, next_end)))
        } else {
            // we're beyond the last range in all sets of ranges
            None
        }
    }
}

/// splits a range into pieces, each of which is contained in one of the ranges in `into_ranges`
pub fn split<const N: usize>(
    range_to_split: (DocCharOffset, DocCharOffset),
    into_ranges: [&[(DocCharOffset, DocCharOffset)]; N],
) -> Vec<(DocCharOffset, DocCharOffset)> {
    let mut result = Vec::new();
    for (indexes, into_range) in join(into_ranges) {
        if indexes.iter().any(|&idx| idx.is_none()) {
            // must be in a range for each splitting range
            continue;
        }

        // must have a nonzero intersection
        let intersection = (
            into_range.start().max(range_to_split.start()),
            into_range.end().min(range_to_split.end()),
        );
        if intersection.0 < intersection.1 {
            // return the intersection
            result.push(intersection);
        }
    }
    result
}

impl MarkdownPlusPlus {
    pub fn print_bounds(&self) {
        self.print_words_bounds();
        self.print_lines_bounds();
        self.print_source_lines_bounds();
        self.print_paragraphs_bounds();
        self.print_text_bounds();
    }

    pub fn print_words_bounds(&self) {
        println!("words: {:?}", self.ranges_text(&self.bounds.words));
    }

    pub fn print_lines_bounds(&self) {
        println!("lines: {:?}", self.ranges_text(&self.bounds.lines));
    }

    pub fn print_source_lines_bounds(&self) {
        println!("source lines: {:?}", self.ranges_text(&self.bounds.source_lines));
    }

    pub fn print_paragraphs_bounds(&self) {
        println!("paragraphs: {:?}", self.ranges_text(&self.bounds.paragraphs));
    }

    pub fn print_text_bounds(&self) {
        println!("text: {:?}", self.ranges_text(&self.bounds.text));
    }

    fn ranges_text(&self, ranges: &[(DocCharOffset, DocCharOffset)]) -> Vec<String> {
        ranges
            .iter()
            .map(|&range| self.buffer[range].to_string())
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod test {
    use lb_rs::model::text::offset_types::DocCharOffset;

    use super::{join, Bounds};
    use crate::tab::markdown_plusplus::{
        bounds::{BoundExt as _, RangesExt as _},
        input::Bound,
    };

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
        assert_eq!(DocCharOffset(4).range_bound(Bound::Word, false, false, &bounds), None);
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
        assert_eq!(DocCharOffset(8).range_bound(Bound::Word, false, false, &bounds), None);
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
        assert_eq!(DocCharOffset(4).range_bound(Bound::Word, true, false, &bounds), None);
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
        assert_eq!(DocCharOffset(8).range_bound(Bound::Word, true, false, &bounds), None);
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
            Some((DocCharOffset(1), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).range_bound(Bound::Word, false, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
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
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(4).range_bound(Bound::Word, true, false, &bounds),
            Some((DocCharOffset(3), DocCharOffset(5)))
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
            DocCharOffset(4)
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
            DocCharOffset(8)
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
        assert_eq!(DocCharOffset(4).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(4));
        assert_eq!(DocCharOffset(5).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
        assert_eq!(DocCharOffset(6).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
        assert_eq!(DocCharOffset(7).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
        assert_eq!(DocCharOffset(8).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(8));
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
            DocCharOffset(7)
        );

        assert_eq!(DocCharOffset(0).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(1).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(2).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(1));
        assert_eq!(DocCharOffset(3).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(3));
        assert_eq!(DocCharOffset(4).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(3));
        assert_eq!(DocCharOffset(5).advance_to_bound(Bound::Word, true, &bounds), DocCharOffset(5));
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
            DocCharOffset(4)
        );
        assert_eq!(
            DocCharOffset(5).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(5)
        );
        assert_eq!(
            DocCharOffset(6).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(6)
        );
        assert_eq!(
            DocCharOffset(7).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(7)
        );
        assert_eq!(
            DocCharOffset(8).advance_to_bound(Bound::Word, false, &bounds),
            DocCharOffset(8)
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
            Some((DocCharOffset(1), DocCharOffset(2)))
        );
        assert_eq!(
            DocCharOffset(3).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(2), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(4).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(6).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(5), DocCharOffset(6)))
        );
        assert_eq!(
            DocCharOffset(7).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(6), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(8).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(9).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(10).char_bound(false, false, &bounds.text),
            Some((DocCharOffset(9), DocCharOffset(10)))
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
            Some((DocCharOffset(2), DocCharOffset(3)))
        );
        assert_eq!(
            DocCharOffset(3).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(4).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(3), DocCharOffset(5)))
        );
        assert_eq!(
            DocCharOffset(5).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(5), DocCharOffset(6)))
        );
        assert_eq!(
            DocCharOffset(6).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(6), DocCharOffset(7)))
        );
        assert_eq!(
            DocCharOffset(7).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(8).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(7), DocCharOffset(9)))
        );
        assert_eq!(
            DocCharOffset(9).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(9), DocCharOffset(10)))
        );
        assert_eq!(
            DocCharOffset(10).char_bound(true, false, &bounds.text),
            Some((DocCharOffset(10), DocCharOffset(11)))
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

    #[test]
    fn find_containing() {
        let ranges: Vec<(DocCharOffset, DocCharOffset)> = vec![
            (1.into(), 3.into()),
            (5.into(), 6.into()),
            (6.into(), 6.into()),
            (6.into(), 7.into()),
        ];

        assert_eq!(ranges.find_containing(0.into(), false, false), (0, 0));
        assert_eq!(ranges.find_containing(1.into(), false, false), (0, 0));
        assert_eq!(ranges.find_containing(2.into(), false, false), (0, 1));
        assert_eq!(ranges.find_containing(3.into(), false, false), (1, 1));
        assert_eq!(ranges.find_containing(4.into(), false, false), (1, 1));
        assert_eq!(ranges.find_containing(5.into(), false, false), (1, 1));
        assert_eq!(ranges.find_containing(6.into(), false, false), (3, 3));
        assert_eq!(ranges.find_containing(7.into(), false, false), (4, 4));
        assert_eq!(ranges.find_containing(8.into(), false, false), (4, 4));

        assert_eq!(ranges.find_containing(0.into(), true, false), (0, 0));
        assert_eq!(ranges.find_containing(1.into(), true, false), (0, 1));
        assert_eq!(ranges.find_containing(2.into(), true, false), (0, 1));
        assert_eq!(ranges.find_containing(3.into(), true, false), (1, 1));
        assert_eq!(ranges.find_containing(4.into(), true, false), (1, 1));
        assert_eq!(ranges.find_containing(5.into(), true, false), (1, 2));
        assert_eq!(ranges.find_containing(6.into(), true, false), (3, 4));
        assert_eq!(ranges.find_containing(7.into(), true, false), (4, 4));
        assert_eq!(ranges.find_containing(8.into(), true, false), (4, 4));

        assert_eq!(ranges.find_containing(0.into(), false, true), (0, 0));
        assert_eq!(ranges.find_containing(1.into(), false, true), (0, 0));
        assert_eq!(ranges.find_containing(2.into(), false, true), (0, 1));
        assert_eq!(ranges.find_containing(3.into(), false, true), (0, 1));
        assert_eq!(ranges.find_containing(4.into(), false, true), (1, 1));
        assert_eq!(ranges.find_containing(5.into(), false, true), (1, 1));
        assert_eq!(ranges.find_containing(6.into(), false, true), (1, 2));
        assert_eq!(ranges.find_containing(7.into(), false, true), (3, 4));
        assert_eq!(ranges.find_containing(8.into(), false, true), (4, 4));

        assert_eq!(ranges.find_containing(0.into(), true, true), (0, 0));
        assert_eq!(ranges.find_containing(1.into(), true, true), (0, 1));
        assert_eq!(ranges.find_containing(2.into(), true, true), (0, 1));
        assert_eq!(ranges.find_containing(3.into(), true, true), (0, 1));
        assert_eq!(ranges.find_containing(4.into(), true, true), (1, 1));
        assert_eq!(ranges.find_containing(5.into(), true, true), (1, 2));
        assert_eq!(ranges.find_containing(6.into(), true, true), (1, 4));
        assert_eq!(ranges.find_containing(7.into(), true, true), (3, 4));
        assert_eq!(ranges.find_containing(8.into(), true, true), (4, 4));
    }

    #[test]
    fn find_intersecting_empty() {
        let ranges: Vec<(DocCharOffset, DocCharOffset)> = vec![
            (1.into(), 3.into()),
            (5.into(), 6.into()),
            (6.into(), 6.into()),
            (6.into(), 7.into()),
        ];

        assert_eq!(ranges.find_intersecting((0.into(), 0.into()), false), (0, 0));
        assert_eq!(ranges.find_intersecting((1.into(), 1.into()), false), (0, 0));
        assert_eq!(ranges.find_intersecting((2.into(), 2.into()), false), (0, 1));
        assert_eq!(ranges.find_intersecting((3.into(), 3.into()), false), (1, 1));
        assert_eq!(ranges.find_intersecting((4.into(), 4.into()), false), (1, 1));
        assert_eq!(ranges.find_intersecting((5.into(), 5.into()), false), (1, 1));
        assert_eq!(ranges.find_intersecting((6.into(), 6.into()), false), (3, 3));
        assert_eq!(ranges.find_intersecting((7.into(), 7.into()), false), (4, 4));
        assert_eq!(ranges.find_intersecting((8.into(), 8.into()), false), (4, 4));

        assert_eq!(ranges.find_intersecting((0.into(), 0.into()), true), (0, 0));
        assert_eq!(ranges.find_intersecting((1.into(), 1.into()), true), (0, 1));
        assert_eq!(ranges.find_intersecting((2.into(), 2.into()), true), (0, 1));
        assert_eq!(ranges.find_intersecting((3.into(), 3.into()), true), (0, 1));
        assert_eq!(ranges.find_intersecting((4.into(), 4.into()), true), (1, 1));
        assert_eq!(ranges.find_intersecting((5.into(), 5.into()), true), (1, 2));
        assert_eq!(ranges.find_intersecting((6.into(), 6.into()), true), (1, 4));
        assert_eq!(ranges.find_intersecting((7.into(), 7.into()), true), (3, 4));
        assert_eq!(ranges.find_intersecting((8.into(), 8.into()), true), (4, 4));
    }

    #[test]
    fn range_join_iter_empty() {
        let a: Vec<(DocCharOffset, DocCharOffset)> = vec![];
        let b: Vec<(DocCharOffset, DocCharOffset)> = vec![];
        let c: Vec<(DocCharOffset, DocCharOffset)> = vec![];

        let result = join([&a, &b, &c]).collect::<Vec<_>>();

        assert_eq!(result, &[]);
    }

    #[test]
    fn range_join_iter() {
        let a = vec![(0.into(), 10.into())];
        let b = vec![(0.into(), 5.into()), (5.into(), 5.into()), (5.into(), 10.into())];
        let c = vec![(3.into(), 7.into())];

        let result = join([&a, &b, &c]).collect::<Vec<_>>();

        assert_eq!(
            result,
            &[
                ([Some(0), Some(0), None], (0.into(), 3.into())),
                ([Some(0), Some(0), Some(0)], (3.into(), 5.into())),
                ([Some(0), Some(1), Some(0)], (5.into(), 5.into())),
                ([Some(0), Some(2), Some(0)], (5.into(), 7.into())),
                ([Some(0), Some(2), None], (7.into(), 10.into())),
            ]
        )
    }
}
