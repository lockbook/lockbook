//! Text units as a platform tokenizer sees them (UIKit's
//! `UITextInputTokenizer`): characters, words, sentences, paragraphs, visual
//! lines, and the document. Each unit is a range; the tokenizer questions are
//! answered the same way for every kind from the units near a position.
//! "Forward" looks after the position, "backward" before it.

use std::borrow::Cow;

use lb_rs::model::text::offset_types::{Byte, Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use super::MdEdit;

type Range = (Grapheme, Grapheme);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Character,
    Word,
    Sentence,
    Paragraph,
    /// A wrap row. It ends before its newline.
    Line,
    Document,
}

/// The unit holding `p` in `direction`; an empty unit holds only its own spot.
pub fn enclosing(units: &[Range], p: Grapheme, backward: bool) -> Option<Range> {
    units.iter().copied().find(|&(start, end)| {
        (start == end && p == start)
            || if backward { start < p && p <= end } else { start <= p && p < end }
    })
}

/// A unit ends at `p` (forward) or starts at `p` (backward).
pub fn at_boundary(units: &[Range], p: Grapheme, backward: bool) -> bool {
    units
        .iter()
        .any(|&(start, end)| if backward { start == p } else { end == p })
}

/// A unit continues after `p` (forward) or precedes it (backward).
pub fn within(units: &[Range], p: Grapheme, backward: bool) -> bool {
    units
        .iter()
        .any(|&(start, end)| if backward { start < p && p <= end } else { start <= p && p < end })
}

/// The nearest unit start or end past `p` in `direction`.
pub fn boundary_from(units: &[Range], p: Grapheme, backward: bool) -> Option<Grapheme> {
    let bounds = units.iter().flat_map(|&(start, end)| [start, end]);
    if backward { bounds.filter(|&b| b < p).max() } else { bounds.filter(|&b| b > p).min() }
}

impl MdEdit {
    pub fn unit_enclosing(&self, unit: Unit, p: Grapheme, backward: bool) -> Option<Range> {
        enclosing(&self.units_near(unit, p), p, backward)
    }

    pub fn unit_at_boundary(&self, unit: Unit, p: Grapheme, backward: bool) -> bool {
        at_boundary(&self.units_near(unit, p), p, backward)
    }

    pub fn unit_within(&self, unit: Unit, p: Grapheme, backward: bool) -> bool {
        within(&self.units_near(unit, p), p, backward)
    }

    pub fn unit_boundary_from(&self, unit: Unit, p: Grapheme, backward: bool) -> Option<Grapheme> {
        if !matches!(unit, Unit::Word | Unit::Sentence) {
            return boundary_from(&self.units_near(unit, p), p, backward);
        }
        // Words and sentences stay inside a paragraph; walk paragraphs outward.
        let paragraphs = &self.renderer.bounds.source_lines;
        let mut i = self.paragraph_index(p);
        loop {
            let found = boundary_from(&self.units_in_paragraph(unit, i), p, backward);
            if found.is_some() {
                return found;
            }
            if backward {
                i = i.checked_sub(1)?;
            } else {
                i += 1;
                if i >= paragraphs.len() {
                    return None;
                }
            }
        }
    }

    fn units_near(&self, unit: Unit, p: Grapheme) -> Cow<'_, [Range]> {
        let last = self.renderer.buffer.current.segs.last_cursor_position();
        match unit {
            Unit::Character => {
                let mut units = Vec::with_capacity(2);
                if p > Grapheme(0) {
                    units.push((p - 1, p));
                }
                if p < last {
                    units.push((p, p + 1));
                }
                Cow::Owned(units)
            }
            Unit::Word | Unit::Sentence => {
                Cow::Owned(self.units_in_paragraph(unit, self.paragraph_index(p)))
            }
            Unit::Paragraph => {
                let i = self.paragraph_index(p);
                Cow::Owned(
                    (i.saturating_sub(1)..=i + 1)
                        .filter_map(|i| self.paragraph_with_newline(i))
                        .collect(),
                )
            }
            Unit::Line => {
                let rows = &self.renderer.bounds.wrap_lines;
                let laid_out = rows.first().is_some_and(|first| first.0 <= p)
                    && rows.last().is_some_and(|row| p <= row.1);
                Cow::Borrowed(if laid_out { rows } else { &self.renderer.bounds.source_lines })
            }
            Unit::Document => Cow::Owned(vec![(Grapheme(0), last)]),
        }
    }

    /// Ranges the platform sees as one glyph with no text to edit inside:
    /// collapsed images and fold chips. A fold's atom is its tag, not the
    /// hidden contents behind it, so a tap beside the chip lands beside it.
    pub fn atoms(&self) -> Vec<Range> {
        let r = &self.renderer;
        let mut atoms = r.bounds.images.clone();
        atoms.extend(r.bounds.folds.iter().map(|f| f.tag));
        atoms.sort_unstable();
        atoms
    }

    /// Words touching an atom become the atom, so a tap's word snap and a
    /// double tap stop at its edges. A link's words are its own.
    fn merge_atoms(&self, units: &mut Vec<Range>, within: Range) {
        let atoms: Vec<Range> = self
            .atoms()
            .into_iter()
            .filter(|atom| atom.intersects(&within, false))
            .collect();
        if atoms.is_empty() {
            return;
        }
        units.retain(|unit| !atoms.iter().any(|atom| unit.intersects(atom, false)));
        units.extend(atoms);
        units.sort_unstable();
    }

    fn paragraph_index(&self, p: Grapheme) -> usize {
        let paragraphs = &self.renderer.bounds.source_lines;
        paragraphs
            .partition_point(|&(start, _)| start <= p)
            .saturating_sub(1)
    }

    /// A source line and the newline after it, as the platform counts a
    /// paragraph.
    fn paragraph_with_newline(&self, i: usize) -> Option<Range> {
        let lines = &self.renderer.bounds.source_lines;
        let &(start, end) = lines.get(i)?;
        let end = lines.get(i + 1).map(|&(next, _)| next).unwrap_or(end);
        Some((start, end))
    }

    /// Words (Unicode word segments with a letter, digit, or symbol) or
    /// sentences (Unicode sentence segments, trailing space and newline
    /// included) of one paragraph, read in place.
    fn units_in_paragraph(&self, unit: Unit, i: usize) -> Vec<Range> {
        let Some((start, end)) = self.paragraph_with_newline(i) else {
            return Vec::new();
        };
        let snap = &self.renderer.buffer.current;
        let (start_byte, end_byte) = snap.segs.range_to_byte((start, end));
        let text = &snap.text[start_byte.0..end_byte.0];
        let to_range = |offset: usize, segment: &str| {
            let lo = snap.segs.byte_to_char_floor(Byte(start_byte.0 + offset));
            let hi = snap
                .segs
                .byte_to_char_ceil(Byte(start_byte.0 + offset + segment.len()));
            (lo, hi)
        };
        match unit {
            Unit::Word => {
                let mut words: Vec<Range> = words(text)
                    .into_iter()
                    .map(|(lo, hi)| to_range(lo, &text[lo..hi]))
                    .collect();
                self.merge_atoms(&mut words, (start, end));
                words
            }
            _ => text
                .split_sentence_bound_indices()
                .map(|(offset, segment)| to_range(offset, segment))
                .collect(),
        }
    }
}

/// Byte ranges of the words in `text`, as Apple's tokenizer finds them. A
/// space-separated run of only punctuation is one word (`**`, `-`, `>`).
/// Otherwise words are runs of letters, digits, and emoji; punctuation
/// touching them splits and is dropped, except an apostrophe between letters
/// (`it’s`) and `.` or `,` between digits (`3.14`).
fn words(text: &str) -> Vec<(usize, usize)> {
    let mut words = Vec::new();
    for (lo, chunk) in chunks(text) {
        if !chunk.chars().any(is_word_char) {
            words.push((lo, lo + chunk.len()));
            continue;
        }
        let chars: Vec<(usize, char)> = chunk.char_indices().collect();
        let mut start = None;
        for (k, &(i, c)) in chars.iter().enumerate() {
            let prev = k.checked_sub(1).map(|k| chars[k].1);
            let next = chars.get(k + 1).map(|&(_, n)| n);
            let joins = is_word_char(c)
                || (matches!(c, '\'' | '’')
                    && prev.is_some_and(char::is_alphabetic)
                    && next.is_some_and(char::is_alphabetic))
                || (matches!(c, '.' | ',')
                    && prev.is_some_and(char::is_numeric)
                    && next.is_some_and(char::is_numeric));
            match (joins, start) {
                (true, None) => start = Some(lo + i),
                (false, Some(s)) => {
                    words.push((s, lo + i));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            words.push((s, lo + chunk.len()));
        }
    }
    words
}

/// Space-separated runs of `text` with their byte offsets.
fn chunks(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split_whitespace()
        .map(move |chunk| (chunk.as_ptr() as usize - text.as_ptr() as usize, chunk))
}

/// Letters, digits, and emoji; not punctuation or symbols like `°` and `§`.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
        || !(c.is_whitespace() || c.is_ascii_punctuation() || is_punctuation_or_symbol(c))
}

fn is_punctuation_or_symbol(c: char) -> bool {
    matches!(c,
        '\u{00A1}'..='\u{00BF}' | '\u{00D7}' | '\u{00F7}'
        | '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{205E}' | '\u{20A0}'..='\u{20CF}'
        | '\u{2100}'..='\u{214F}' | '\u{2190}'..='\u{23FF}' | '\u{2500}'..='\u{25FF}'
        | '\u{3001}'..='\u{3003}' | '\u{3008}'..='\u{3011}' | '\u{FF01}'..='\u{FF0F}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(i: usize) -> Grapheme {
        Grapheme(i)
    }

    /// "screen.\n\nI like" soft-wrapped: row 0..4 "scre", row 4..7 "en.",
    /// empty row 8..8, row 9..15 "I like".
    fn rows() -> Vec<Range> {
        vec![(g(0), g(4)), (g(4), g(7)), (g(8), g(8)), (g(9), g(15))]
    }

    #[test]
    fn a_hard_line_end_is_before_the_newline() {
        assert!(at_boundary(&rows(), g(7), false));
        assert!(!at_boundary(&rows(), g(7), true));
        assert!(!within(&rows(), g(7), false));
        assert!(within(&rows(), g(7), true));
        assert_eq!(enclosing(&rows(), g(7), true), Some((g(4), g(7))));
        assert_eq!(enclosing(&rows(), g(7), false), None);
    }

    #[test]
    fn a_soft_wrap_ends_one_row_and_starts_the_next() {
        assert!(at_boundary(&rows(), g(4), false));
        assert!(at_boundary(&rows(), g(4), true));
        assert_eq!(enclosing(&rows(), g(4), true), Some((g(0), g(4))));
        assert_eq!(enclosing(&rows(), g(4), false), Some((g(4), g(7))));
    }

    #[test]
    fn a_blank_line_is_its_own_row() {
        assert!(at_boundary(&rows(), g(8), false));
        assert!(at_boundary(&rows(), g(8), true));
        assert_eq!(enclosing(&rows(), g(8), false), Some((g(8), g(8))));
        assert_eq!(enclosing(&rows(), g(8), true), Some((g(8), g(8))));
    }

    #[test]
    fn a_boundary_is_the_nearest_start_or_end() {
        assert_eq!(boundary_from(&rows(), g(2), false), Some(g(4)));
        assert_eq!(boundary_from(&rows(), g(4), false), Some(g(7)));
        assert_eq!(boundary_from(&rows(), g(7), false), Some(g(8)));
        assert_eq!(boundary_from(&rows(), g(15), false), None);
        assert_eq!(boundary_from(&rows(), g(9), true), Some(g(8)));
        assert_eq!(boundary_from(&rows(), g(0), true), None);
    }

    #[test]
    fn sentences_keep_trailing_space_and_a_blank_line_is_one() {
        let text = "One. Two screen.\n\nI like";
        let sentences: Vec<&str> = text.split_sentence_bounds().collect();
        assert_eq!(sentences, ["One. ", "Two screen.\n", "\n", "I like"]);
    }

    fn word_texts(text: &str) -> Vec<&str> {
        words(text)
            .into_iter()
            .map(|(lo, hi)| &text[lo..hi])
            .collect()
    }

    /// Rows from Apple's tokenizer run on device over `x <context> y`.
    #[test]
    fn words_match_apples_tokenizer() {
        let cases: &[(&str, &[&str])] = &[
            ("!", &["!"]),
            ("!!", &["!!"]),
            ("((", &["(("]),
            ("…", &["…"]),
            ("ab!", &["ab"]),
            ("!ab", &["ab"]),
            ("ab!cd", &["ab", "cd"]),
            ("ab_cd", &["ab", "cd"]),
            ("ab.cd", &["ab", "cd"]),
            ("ab'cd", &["ab'cd"]),
            ("ab’cd", &["ab’cd"]),
            ("12'34", &["12", "34"]),
            ("12.34", &["12.34"]),
            ("12,34", &["12,34"]),
            ("12:34", &["12", "34"]),
            ("ab°cd", &["ab", "cd"]),
            ("ab··cd", &["ab", "cd"]),
        ];
        for (context, expected) in cases {
            let text = format!("x {context} y");
            let mut want = vec!["x"];
            want.extend_from_slice(expected);
            want.push("y");
            assert_eq!(word_texts(&text), want, "{context}");
        }
    }

    #[test]
    fn words_in_markdown() {
        assert_eq!(word_texts("## Bottom"), ["##", "Bottom"]);
        assert_eq!(word_texts("and __paste__ them"), ["and", "paste", "them"]);
        assert_eq!(word_texts("long ^ by^ to"), ["long", "^", "by", "to"]);
        assert_eq!(word_texts("docs/logo.svg) 👍🏻 ok"), ["docs", "logo", "svg", "👍🏻", "ok"]);
        assert_eq!(word_texts("> quote\n- ok"), [">", "quote", "-", "ok"]);
    }
}
