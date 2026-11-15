pub mod buffer;
pub mod offset_types;
pub mod operation_types;
pub mod unicode_segs;
pub mod units;
pub mod words;

use offset_types::{Byte, RangeExt as _};
use operation_types::Replace;

use words::Words;

/// The edits from `from` to `to`, pairing their unicode words.
pub fn diff(from: &str, to: &str) -> Vec<Replace> {
    diff_words(from, to, Words::Unicode)
}

/// The edits from `from` to `to`, pairing their `words`.
pub fn diff_words(from: &str, to: &str, words: Words) -> Vec<Replace> {
    let mut result = Vec::new();

    let from_segs = unicode_segs::calc(from);
    let to_segs = unicode_segs::calc(to);

    let from_words = words.bounds(from);
    let to_words = words.bounds(to);
    let tokens = |text: &str, bounds: &[Byte]| -> Vec<String> {
        bounds
            .windows(2)
            .map(|w| text[w[0].0..w[1].0].to_string())
            .collect()
    };
    let (from_tokens, to_tokens) = (tokens(from, &from_words), tokens(to, &to_words));
    let ops = similar::capture_diff_slices(similar::Algorithm::Myers, &from_tokens, &to_tokens);

    // positions come from walking the ops' lengths; similar's own indices can disagree with
    // the run before them (an insert after an equal may name the equal's first token)
    let (mut old_token, mut new_token) = (0, 0);
    for diff_op in &ops {
        let (tag, old_range, new_range) = diff_op.as_tag_tuple();
        let (old_start, new_start) = (old_token, new_token);
        old_token += old_range.len();
        new_token += new_range.len();
        if tag == similar::DiffTag::Equal {
            continue;
        }

        let old_start = from_segs.byte_to_char_floor(from_words[old_start]);
        let old_end = from_segs.byte_to_char_floor(from_words[old_token]);
        let new_text_range = to_segs.range_to_byte((
            to_segs.byte_to_char_floor(to_words[new_start]),
            to_segs.byte_to_char_floor(to_words[new_token]),
        ));
        let new_text = &to[new_text_range.start().0..new_text_range.end().0];

        match result.last_mut() {
            Some(Replace { range, text }) if range.1 == old_start => {
                range.1 = old_end;
                text.push_str(new_text);
            }
            _ => result.push(Replace { range: (old_start, old_end), text: new_text.into() }),
        }
    }
    result
}

#[cfg(test)]
mod test {
    use super::words::Words;
    use rand::rngs::StdRng;
    use rand::{Rng as _, SeedableRng as _};

    #[test]
    fn diff_full_replace() {
        let from = "Hello";
        let to = "Goodbye";

        let result = super::diff(from, to);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].range, (0.into(), 5.into()));
        assert_eq!(result[0].text, "Goodbye");
    }

    #[test]
    fn diff_partial_replace() {
        let from = "Hello, world!";
        let to = "Hello, Rust!";

        let result = super::diff(from, to);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].range, (7.into(), 12.into()));
        assert_eq!(result[0].text, "Rust");
    }

    // word segmentation skips the format char and pairs the regional indicators into one word,
    // putting a word boundary inside the flag grapheme
    #[test]
    fn diff_word_boundary_inside_grapheme() {
        let from = "\u{1f1f8}";
        let to = "\u{1f1f8}\u{feff}\u{1f1fa}\u{1f1f8}";

        let result = super::diff(from, to);
        let from_graphemes = super::unicode_segs::calc(from).grapheme_indexes.len() - 1;
        for op in &result {
            assert!(op.range.0 <= op.range.1 && op.range.1.0 <= from_graphemes);
        }
    }

    #[test]
    fn diff_insert_after_emoji() {
        assert_round_trip("foo 👋 foo\n", "foo 👋foo  foo\n");
    }

    #[test]
    fn diff_fuzz() {
        let mut rng = StdRng::seed_from_u64(0);
        for _ in 0..1000 {
            let len = rng.gen_range(0..10);
            let from = rand_str(&mut rng, len);
            let len = rng.gen_range(0..10);
            let to = rand_str(&mut rng, len);
            assert_round_trip(&from, &to);
        }
        // few distinct words, so the diff keeps runs equal
        let words = ["foo", "bar", " ", "  ", "\n", "👋", "Ж", "é", "🇸🇺"];
        for _ in 0..10000 {
            let words_str = |rng: &mut StdRng| -> String {
                let len = rng.gen_range(0..12);
                (0..len)
                    .map(|_| words[rng.gen_range(0..words.len())])
                    .collect()
            };
            let from = words_str(&mut rng);
            let to = words_str(&mut rng);
            assert_round_trip(&from, &to);
        }
    }

    #[test]
    fn diff_fuzz_markdown() {
        let mut rng = StdRng::seed_from_u64(0);
        let words = [
            "![a](a.png)",
            "[",
            "](",
            ")",
            "_",
            "**",
            "`",
            "[[",
            "]]",
            "\\",
            "$",
            " ",
            "\n",
            "x",
            "é",
            "\u{301}",
            "🇸🇺",
            "https://x.test/a",
        ];
        for _ in 0..10000 {
            let words_str = |rng: &mut StdRng| -> String {
                let len = rng.gen_range(0..16);
                (0..len)
                    .map(|_| words[rng.gen_range(0..words.len())])
                    .collect()
            };
            let from = words_str(&mut rng);
            let to = words_str(&mut rng);
            assert_round_trip_with(&from, &to, Words::Markdown);
        }
    }

    /// `diff(from, to)` applied to `from` gives `to`.
    fn assert_round_trip(from: &str, to: &str) {
        assert_round_trip_with(from, to, Words::Unicode);
    }

    /// `diff_words(from, to, words)` applied to `from` gives `to`.
    fn assert_round_trip_with(from: &str, to: &str, words: Words) {
        let segs = super::unicode_segs::calc(from);
        let mut applied = from.to_string();
        for op in super::diff_words(from, to, words).iter().rev() {
            let (start, end) = segs.range_to_byte(op.range);
            applied.replace_range(start.0..end.0, &op.text);
        }
        assert_eq!(applied, to, "diff({from:?}, {to:?})");
    }

    fn rand_str(rng: &mut StdRng, length: usize) -> String {
        let unicode_string: String = (0..length)
            .map(|_| {
                let code_point = rng.gen_range(0x0020..=0xD7FF);
                std::char::from_u32(code_point).unwrap_or('?')
            })
            .collect();
        unicode_string
    }
}
