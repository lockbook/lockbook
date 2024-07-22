use crate::tab::markdown_editor::appearance::{Appearance, CaptureCondition};
use crate::tab::markdown_editor::ast::{Ast, AstTextRange, AstTextRangeType};
use crate::tab::markdown_editor::buffer::SubBuffer;
use crate::tab::markdown_editor::galleys::Galleys;
use crate::tab::markdown_editor::input::canonical::Bound;
use crate::tab::markdown_editor::input::capture::CaptureState;
use crate::tab::markdown_editor::input::cursor::Cursor;
use crate::tab::markdown_editor::offset_types::{
    DocByteOffset, DocCharOffset, RangeExt, RelByteOffset,
};
use crate::tab::markdown_editor::style::{BlockNodeType, InlineNodeType, MarkdownNodeType};
use crate::tab::markdown_editor::unicode_segs::UnicodeSegs;
use crate::tab::markdown_editor::Editor;
use egui::epaint::text::cursor::RCursor;
use linkify::LinkFinder;
use std::cmp::Ordering;
use tldextract::{TldExtractor, TldOption};
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
    /// AST text ranges are separated according to markdown syntax and annotated with a type (head/tail/text) and a list of
    /// ancestor AST nodes so that text can be associated with its position in the AST. An AST node will always have a head
    /// or tail range containing the syntax characters that define the node. Text ranges are between the head and tail.
    /// * Documents may have no AST text ranges.
    /// * AST text ranges cannot be empty.
    /// * AST text ranges can touch.
    pub ast: AstTextRanges,

    /// Words are separated by UAX#29 (Unicode Standard Annex #29) word boundaries and do not contain whitespace. Some
    /// punctuation marks count as words. Markdown syntax sequences count as single words.
    /// * Documents may have no words.
    /// * Words cannot be empty.
    /// * Words can touch.
    pub words: Words,

    /// Lines are separated by newline characters or by line wrap.
    /// * Documents have at least one line.
    /// * Lines can be empty.
    /// * Lines can touch.
    pub lines: Lines,

    /// Paragraphs are separated by newline characters.
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

    /// Plain text links are styled and clickable but aren't markdown links.
    /// * Documents may have no links.
    /// * Links cannot be empty.
    /// * Links cannot touch.
    pub links: PlainTextLinks,
}

pub fn calc_ast(ast: &Ast) -> AstTextRanges {
    ast.iter_text_ranges().collect()
}

pub fn calc_words(
    buffer: &SubBuffer, ast: &Ast, ast_ranges: &AstTextRanges, appearance: &Appearance,
) -> Words {
    let mut result = vec![];

    for text_range in ast_ranges {
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

pub fn calc_lines(galleys: &Galleys, ast: &AstTextRanges, text: &Text) -> Lines {
    let mut result = vec![];
    let mut text_range_iter = ast.iter();
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

    // ios testing hack: break lines every 5 characters
    // let mut new_result = vec![];
    // for (start, end) in result {
    //     let mut start = start;
    //     while end - start > 5 {
    //         new_result.push((start, start + 5));
    //         start += 5;
    //     }
    //     new_result.push((start, end));
    // }

    result
}

pub fn calc_paragraphs(buffer: &SubBuffer) -> Paragraphs {
    let mut result = vec![];

    let carriage_return_matches = buffer
        .text
        .match_indices('\r')
        .map(|(idx, _)| DocByteOffset(idx))
        .collect::<HashSet<_>>();
    let line_feed_matches = buffer
        .text
        .match_indices('\n')
        .map(|(idx, _)| DocByteOffset(idx))
        .filter(|&byte_offset| !carriage_return_matches.contains(&(byte_offset - 1)));

    let mut newline_matches = Vec::new();
    newline_matches.extend(line_feed_matches);
    newline_matches.extend(carriage_return_matches);
    newline_matches.sort();

    let mut prev_char_offset = DocCharOffset(0);
    for byte_offset in newline_matches {
        let char_offset = buffer.segs.offset_to_char(byte_offset);

        // note: paragraphs can be empty
        result.push((prev_char_offset, char_offset));

        prev_char_offset = char_offset + 1 // skip the matched newline;
    }
    result.push((prev_char_offset, buffer.segs.last_cursor_position()));

    result
}

pub fn calc_text(
    ast: &Ast, ast_ranges: &AstTextRanges, paragraphs: &Paragraphs, appearance: &Appearance,
    segs: &UnicodeSegs, cursor: Cursor, capture: &CaptureState,
) -> Text {
    let mut result = vec![];
    let mut last_range_pushed = false;
    for (i, text_range) in ast_ranges.iter().enumerate() {
        let captured = capture.captured(cursor, paragraphs, ast, ast_ranges, i, appearance);

        let this_range_pushed = if !captured {
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

pub fn calc_links(buffer: &SubBuffer, text: &Text, ast: &Ast) -> PlainTextLinks {
    let finder = {
        let mut this = LinkFinder::new();
        this.kinds(&[linkify::LinkKind::Url])
            .url_must_have_scheme(false)
            .url_can_be_iri(false); // ignore links with international characters for phishing prevention
        this
    };

    let mut result = vec![];
    for &text_range in text {
        'spans: for span in finder.spans(&buffer[text_range]) {
            let link_range = (text_range.0 + span.start(), text_range.0 + span.end());

            if span.kind().is_none() {
                continue;
            }

            let link_text = if buffer[link_range].contains("://") {
                buffer[link_range].to_string()
            } else {
                format!("http://{}", &buffer[link_range])
            };

            match TldExtractor::new(TldOption::default()).extract(&link_text) {
                Ok(tld) => {
                    // the last one of these must be a top level domain
                    if let Some(ref d) = tld.suffix {
                        if !tld::exist(d) {
                            continue;
                        }
                    } else if let Some(ref d) = tld.domain {
                        if !tld::exist(d) {
                            continue;
                        }
                    } else if let Some(ref d) = tld.subdomain {
                        if !tld::exist(d) {
                            continue;
                        }
                    }
                }
                Err(_) => {
                    continue;
                }
            }

            // ignore links in code blocks because field references or method invocations can look like URLs
            for node in &ast.nodes {
                let node_type_ignores_links = node.node_type.node_type()
                    == MarkdownNodeType::Block(BlockNodeType::Code)
                    || node.node_type.node_type() == MarkdownNodeType::Inline(InlineNodeType::Code);
                if node_type_ignores_links && node.range.intersects(&link_range, false) {
                    continue 'spans;
                }
            }

            result.push(link_range);
        }
    }

    result
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

    pub fn ast_ranges(&self) -> Vec<(DocCharOffset, DocCharOffset)> {
        self.ast.iter().map(|text_range| text_range.range).collect()
    }
}

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

impl DocCharOffset {
    /// Returns the range in the direction of `backwards` from offset `self`. `jump` is used to control behavior when
    /// `self` is at a boundary in the direction of `backwards`. When `backwards` and `jump` are false and `self` is at
    /// the end of a range, returns that range. When `backwards` is `false` but `jump` is true and `self` is at the end
    /// of a range, returns the next range. If `jump` is true, advancing beyond the first or last character in the doc
    /// will return None, otherwise it will return the first or last range in the doc.
    ///
    /// For example, `jump` would be set to `true` when implementing alt+left/right behavior, which should always move
    /// the cursor to the next word, but set to `false` when implementing cmd+left/right behavior, which should not
    /// move the cursor if it is already at the line bound in the same direction.
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
                    Some((range_before.end(), self))
                } else {
                    Some((self, range_after.start()))
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
    /// Efficiently finds the possibly empty (inclusive, exclusive) range of ranges that contain `offset`.
    /// When no ranges contain `offset`, result.start() == result.end() == the index of the first range after `offset`.
    fn find_containing(
        &self, offset: DocCharOffset, start_inclusive: bool, end_inclusive: bool,
    ) -> (usize, usize);

    /// Efficiently finds the possibly empty (inclusive, exclusive) range of ranges that are contained by `range`.
    /// When no ranges are contained by `range`, result.start() == result.end() == the index of the first range after `range`.
    fn find_contained(
        &self, range: (DocCharOffset, DocCharOffset), start_inclusive: bool, end_inclusive: bool,
    ) -> (usize, usize);

    /// Efficiently finds the possibly empty (inclusive, exclusive) range of ranges that intersect `range`.
    /// When no ranges intersect `range`, result.start() == result.end() == the index of the first range after `range`.
    fn find_intersecting(
        &self, range: (DocCharOffset, DocCharOffset), allow_empty: bool,
    ) -> (usize, usize);
}

impl<Range: RangeExt<DocCharOffset>> RangesExt for Vec<Range> {
    fn find_containing(
        &self, offset: DocCharOffset, start_inclusive: bool, end_inclusive: bool,
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
        &self, range: (DocCharOffset, DocCharOffset), start_inclusive: bool, end_inclusive: bool,
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
        &self, range: (DocCharOffset, DocCharOffset), allow_empty: bool,
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

impl Editor {
    pub fn print_bounds(&self) {
        self.print_ast_bounds();
        self.print_words_bounds();
        self.print_lines_bounds();
        self.print_paragraphs_bounds();
        self.print_text_bounds();
        self.print_links_bounds();
    }

    pub fn print_ast_bounds(&self) {
        println!(
            "ast: {:?}",
            self.ranges_text(&self.bounds.ast.iter().map(|r| r.range).collect::<Vec<_>>())
        );
    }

    pub fn print_words_bounds(&self) {
        println!("words: {:?}", self.ranges_text(&self.bounds.words));
    }

    pub fn print_lines_bounds(&self) {
        println!("lines: {:?}", self.ranges_text(&self.bounds.lines));
    }

    pub fn print_paragraphs_bounds(&self) {
        println!("paragraphs: {:?}", self.ranges_text(&self.bounds.paragraphs));
    }

    pub fn print_text_bounds(&self) {
        println!("text: {:?}", self.ranges_text(&self.bounds.text));
    }

    pub fn print_links_bounds(&self) {
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
    use super::{join, Bounds, RangesExt};
    use crate::tab::markdown_editor::{input::canonical::Bound, offset_types::DocCharOffset};

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
