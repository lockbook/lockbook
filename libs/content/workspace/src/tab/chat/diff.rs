//! What an edit changed, word by word. A model asked to fix one typo often
//! replaces the whole paragraph; this finds the word.

use similar::{ChangeTag, DiffTag, TextDiff};
use unicode_segmentation::UnicodeSegmentation as _;

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Same(String),
    Gone(String),
    New(String),
}

impl Change {
    pub fn text(&self) -> &str {
        let (Self::Same(text) | Self::Gone(text) | Self::New(text)) = self;
        text
    }
}

/// When less of the shorter text than this survives, word-by-word marks
/// are noise and the edit reads better as the old text, then the new.
const KEPT_FLOOR: f32 = 0.25;

/// The changes from `old` to `new`, in reading order. The `Same` and `Gone`
/// pieces spell `old`; the `Same` and `New` pieces spell `new`.
///
/// Lines that did not change hold their place, so no change reaches across
/// one; the lines between them are compared word by word.
pub fn changes(old: &str, new: &str) -> Vec<Change> {
    let (old_lines, new_lines) = (lines(old), lines(new));
    let mut out = Vec::new();
    for op in TextDiff::from_slices(&old_lines, &new_lines).ops() {
        let (tag, was, is) = op.as_tag_tuple();
        let (was, is) = (old_lines[was].concat(), new_lines[is].concat());
        match tag {
            DiffTag::Equal => push(&mut out, Change::Same(was)),
            DiffTag::Delete => push(&mut out, Change::Gone(was)),
            DiffTag::Insert => push(&mut out, Change::New(is)),
            DiffTag::Replace => {
                for piece in words(&was, &is) {
                    push(&mut out, piece);
                }
            }
        }
    }
    out
}

/// The lines of a text, each with the blank lines after it: a blank line
/// two texts share says nothing about where their paragraphs line up.
fn lines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut end) = (0, 0);
    for line in text.split_inclusive('\n') {
        if end > start && !line.trim().is_empty() {
            out.push(&text[start..end]);
            start = end;
        }
        end += line.len();
    }
    if end > start {
        out.push(&text[start..end]);
    }
    out
}

/// The words, spaces, and marks of a text. A task's box is one of them, so
/// checking it off is not a change one character wide.
fn tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for (at, word) in text.split_word_bound_indices() {
        let boxed =
            word == "]" && at >= 2 && matches!(text.get(at - 2..at), Some("[ " | "[x" | "[X"));
        if boxed {
            out.truncate(out.len() - 2);
            out.push(&text[at - 2..=at]);
        } else {
            out.push(word);
        }
    }
    out
}

/// Lines that changed, compared word by word.
fn words(old: &str, new: &str) -> Vec<Change> {
    let (old_words, new_words) = (tokens(old), tokens(new));
    let diff = TextDiff::from_slices(&old_words, &new_words);
    let (mut tags, texts): (Vec<ChangeTag>, Vec<&str>) = diff
        .iter_all_changes()
        .map(|change| (change.tag(), change.value()))
        .unzip();
    align(&mut tags, &texts);
    let is_word = |text: &str| text.chars().any(char::is_alphanumeric);

    // A change of punctuation alone would be a mark one character wide, as
    // easy to miss as the edit it stands for. It takes the word beside it,
    // which then reads as removed and added.
    let mut widened = vec![false; tags.len()];
    let mut i = 0;
    while i < tags.len() {
        let start = i;
        while i < tags.len() && tags[i] != ChangeTag::Equal {
            i += 1;
        }
        if i == start {
            i += 1;
        } else if !texts[start..i].iter().any(|text| is_word(text)) {
            let beside = [start.checked_sub(1), Some(i).filter(|&i| i < tags.len())];
            if let Some(word) = beside.into_iter().flatten().find(|&j| is_word(texts[j])) {
                widened[word] = true;
            }
        }
    }

    let size = |text: &str| text.chars().filter(|c| !c.is_whitespace()).count();
    let mut out: Vec<Change> = Vec::new();
    let mut kept = 0;
    for ((tag, text), widened) in tags.into_iter().zip(texts).zip(widened) {
        match tag {
            ChangeTag::Equal if widened => {
                push(&mut out, Change::Gone(text.to_string()));
                push(&mut out, Change::New(text.to_string()));
            }
            ChangeTag::Equal => {
                kept += size(text);
                push(&mut out, Change::Same(text.to_string()));
            }
            ChangeTag::Delete => push(&mut out, Change::Gone(text.to_string())),
            ChangeTag::Insert => push(&mut out, Change::New(text.to_string())),
        }
    }
    let shorter = size(old).min(size(new));
    if shorter > 0 && (kept as f32) < shorter as f32 * KEPT_FLOOR {
        return vec![Change::Gone(old.to_string()), Change::New(new.to_string())];
    }
    absorb_chaff(out)
}

/// Moves each addition or removal that stands alone to where it reads
/// best. One can often sit a few words either way and spell the same two
/// texts, but only one of those places is a whole sentence or a whole line.
fn align(tags: &mut [ChangeTag], texts: &[&str]) {
    let equal = |tags: &[ChangeTag], at: usize| tags.get(at) == Some(&ChangeTag::Equal);
    let mut i = 0;
    while i < tags.len() {
        let tag = tags[i];
        let (mut start, mut end) = (i, i);
        while end < tags.len() && tags[end] == tag {
            end += 1;
        }
        i = end;
        let alone =
            (start == 0 || equal(tags, start - 1)) && (end == tags.len() || equal(tags, end));
        if tag == ChangeTag::Equal || !alone {
            continue;
        }
        // As far back as it goes, then forward a word at a time.
        while start > 0 && equal(tags, start - 1) && texts[start - 1] == texts[end - 1] {
            tags.swap(start - 1, end - 1);
            (start, end) = (start - 1, end - 1);
        }
        let mut best = (fit(tags, texts, start, end), start);
        while equal(tags, end) && texts[start] == texts[end] {
            tags.swap(start, end);
            (start, end) = (start + 1, end + 1);
            // On a tie the later place wins, where a word trails its space.
            let fit = fit(tags, texts, start, end);
            if fit >= best.0 {
                best = (fit, start);
            }
        }
        while start > best.1 {
            tags.swap(start - 1, end - 1);
            (start, end) = (start - 1, end - 1);
        }
        i = end;
    }
}

/// How well a change reads sitting at `start..end`: how clean a break each
/// of its ends falls on.
fn fit(tags: &[ChangeTag], texts: &[&str], start: usize, end: usize) -> u8 {
    let equal = |at: &usize| tags[*at] == ChangeTag::Equal;
    // Up to two unchanged words on each side; none means an edge.
    let from = (0..start).rev().take(2).take_while(equal).last();
    let to = (end..texts.len()).take(2).take_while(equal).last();
    let run = &texts[start..end];
    seam(&texts[from.unwrap_or(start)..start], run)
        + seam(run, &texts[end..to.map_or(end, |at| at + 1)])
}

/// How clean a break falls between `one` and `two`: best at an edge of the
/// text or after a blank line, worst inside a word.
fn seam(one: &[&str], two: &[&str]) -> u8 {
    let (Some(last), Some(first)) = (one.last(), two.first()) else { return 7 };
    let ends_line = |text: &str| text.ends_with('\n');
    let (a, b) = (last.chars().next_back().unwrap_or(' '), first.chars().next().unwrap_or(' '));
    if ends_line(last) && one.len() > 1 && ends_line(one[one.len() - 2]) {
        6
    } else if ends_line(last) {
        5
    } else if ends_line(first) {
        4
    } else if !a.is_alphanumeric() && !a.is_whitespace() && b.is_whitespace() {
        // The end of a sentence or a clause.
        3
    } else if a.is_whitespace() || b.is_whitespace() {
        2
    } else {
        u8::from(!a.is_alphanumeric() || !b.is_alphanumeric())
    }
}

/// Appends a piece, joining it to the last of its kind. A change keeps what
/// went ahead of what came.
fn push(out: &mut Vec<Change>, piece: Change) {
    match (out.last_mut(), &piece) {
        (Some(Change::Same(last)), Change::Same(text))
        | (Some(Change::Gone(last)), Change::Gone(text))
        | (Some(Change::New(last)), Change::New(text)) => last.push_str(text),
        (Some(Change::New(_)), Change::Gone(text)) => {
            let came = out.pop().expect("a last piece");
            push(out, Change::Gone(text.clone()));
            out.push(came);
        }
        _ => out.push(piece),
    }
}

/// Unchanged text no longer than the changes on both sides of it is a
/// coincidence, not context: "the food is out" for "dinner is served"
/// should not leave "is" standing between two halves. It joins them.
fn absorb_chaff(pieces: Vec<Change>) -> Vec<Change> {
    // (what went, what came) around each stretch of unchanged text.
    let mut edits: Vec<(String, String)> = vec![Default::default()];
    let mut sames: Vec<String> = Vec::new();
    for piece in pieces {
        match piece {
            Change::Gone(text) => edits.last_mut().expect("an edit").0.push_str(&text),
            Change::New(text) => edits.last_mut().expect("an edit").1.push_str(&text),
            Change::Same(text) => {
                sames.push(text);
                edits.push(Default::default());
            }
        }
    }
    let size = |edit: &(String, String)| edit.0.chars().count().max(edit.1.chars().count());
    let mut i = 0;
    while i < sames.len() {
        let (before, after) = (&edits[i], &edits[i + 1]);
        let same = sames[i].chars().count();
        // Joining must leave both a removal and an addition with words of
        // their own, or it would only invent a change to the text between.
        // A change that took in a line break would read as two lines mixed.
        let both = [(&before.0, &after.0), (&before.1, &after.1)]
            .iter()
            .all(|(a, b)| !a.trim().is_empty() || !b.trim().is_empty());
        let one_line = !sames[i].contains('\n');
        if both && one_line && same <= size(before) && same <= size(after) {
            let (same, after) = (sames.remove(i), edits.remove(i + 1));
            edits[i].0 = format!("{}{same}{}", edits[i].0, after.0);
            edits[i].1 = format!("{}{same}{}", edits[i].1, after.1);
            // The joined change is larger; what came before may now join it.
            i = i.saturating_sub(1);
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    let mut sames = sames.into_iter();
    for (gone, came) in edits {
        if !gone.is_empty() {
            out.push(Change::Gone(gone));
        }
        if !came.is_empty() {
            out.push(Change::New(came));
        }
        out.extend(sames.next().map(Change::Same));
    }
    out
}

/// The changes with long unchanged stretches cut down to about `words`
/// words on each side of a change, an ellipsis standing for the rest.
pub fn in_context(changes: Vec<Change>, words: usize) -> Vec<Change> {
    let last = changes.len().saturating_sub(1);
    changes
        .into_iter()
        .enumerate()
        .map(|(i, change)| match change {
            Change::Same(text) => Change::Same(context(&text, words, i > 0, i < last)),
            other => other,
        })
        .collect()
}

/// `text` between two changes: what follows the one before it (when
/// `after`) and what leads up to the next (when `before`).
fn context(text: &str, words: usize, after: bool, before: bool) -> String {
    let head = if after { head(text, words) } else { 0 };
    let tail = if before { tail(text, words) } else { text.len() };
    // Hiding less than one side shows is not worth an ellipsis.
    if head >= tail || word_spans(&text[head..tail]).len() < words {
        return text.to_string();
    }
    let (head, hidden, tail) = (&text[..head], &text[head..tail], &text[tail..]);
    // A cut inside a line trails off; one between lines is a line itself.
    let head_open = !head.is_empty() && !head.ends_with('\n');
    let tail_open = !tail.is_empty() && !hidden.ends_with('\n');
    let mark = match (head_open, tail_open) {
        (true, true) if hidden.contains('\n') => " …\n… ",
        (true, true) => " … ",
        (true, false) if tail.is_empty() => " …",
        (true, false) => " …\n",
        (false, true) => "… ",
        (false, false) if tail.is_empty() => "…",
        (false, false) => "…\n",
    };
    format!("{head}{mark}{tail}")
}

/// How much of `text` to keep after a change: whole lines while they hold
/// no more than `words` words, or that many words of a longer first line.
fn head(text: &str, words: usize) -> usize {
    let (mut kept, mut left) = (0, words);
    for line in text.split_inclusive('\n') {
        let spans = word_spans(line);
        if spans.len() > left {
            if kept == 0 {
                return spans[..words].last().map_or(0, |span| span.1);
            }
            break;
        }
        left -= spans.len();
        kept += line.len();
    }
    kept
}

/// Where in `text` to keep from before a change: `head`, mirrored.
fn tail(text: &str, words: usize) -> usize {
    let (mut from, mut left) = (text.len(), words);
    for line in text.split_inclusive('\n').rev() {
        let spans = word_spans(line);
        if spans.len() > left {
            if from == text.len() {
                let within = spans.get(spans.len() - words);
                return from - line.len() + within.map_or(line.len(), |span| span.0);
            }
            break;
        }
        left -= spans.len();
        from -= line.len();
    }
    from
}

/// Where the words of `text` start and end. A word holds a letter or a
/// digit; a list's dash or a table's bar is not one.
fn word_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (at, ch) in text.char_indices().chain([(text.len(), ' ')]) {
        match (ch.is_whitespace(), start) {
            (false, None) => start = Some(at),
            (true, Some(from)) => {
                if text[from..at].chars().any(char::is_alphanumeric) {
                    spans.push((from, at));
                }
                start = None;
            }
            _ => {}
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(text: &str) -> Change {
        Change::Same(text.into())
    }
    fn gone(text: &str) -> Change {
        Change::Gone(text.into())
    }
    fn new(text: &str) -> Change {
        Change::New(text.into())
    }

    /// The case that started this: one word fixed in a paragraph that the
    /// model sent back whole.
    #[test]
    fn a_fixed_typo_is_one_word() {
        let old = "We will recieve the package tomorrow, and unpack it then.";
        let fixed = "We will receive the package tomorrow, and unpack it then.";
        assert_eq!(
            changes(old, fixed),
            [
                same("We will "),
                gone("recieve"),
                new("receive"),
                same(" the package tomorrow, and unpack it then.")
            ]
        );
    }

    #[test]
    fn a_replaced_phrase_stays_together() {
        assert_eq!(
            changes("see the quick brown fox run", "see the slow red fox run"),
            [same("see the "), gone("quick brown"), new("slow red"), same(" fox run")]
        );
    }

    /// A lone changed comma would be a one-character mark, as easy to miss
    /// as the edit it stands for; it takes its word with it.
    #[test]
    fn changed_punctuation_marks_its_word() {
        assert_eq!(
            changes("it works well.", "it works well!"),
            [same("it works "), gone("well."), new("well!")]
        );
        assert_eq!(
            changes("first second third", "first, second third"),
            [gone("first"), new("first,"), same(" second third")]
        );
    }

    /// Text added to a sentence leaves the sentence standing, however much
    /// longer the result is.
    #[test]
    fn an_addition_keeps_what_it_was_added_to() {
        assert_eq!(
            changes(
                "Let purpose be your bouncer.",
                "Let purpose be your bouncer: it decides what is in and what is out."
            ),
            [
                same("Let purpose be your bouncer"),
                new(": it decides what is in and what is out"),
                same(".")
            ]
        );
        let grown = format!("One line.{}", " And much more after it.".repeat(8));
        assert_eq!(changes("One line.", &grown)[0], same("One line."));
    }

    /// Small words that two neighboring changes happen to share are part of
    /// the change, not islands between its halves.
    #[test]
    fn a_coincidental_match_joins_the_change_around_it() {
        assert_eq!(
            changes(
                "keep the music low until the food is out.",
                "keep the music low until dinner is served."
            ),
            [
                same("keep the music low until "),
                gone("the food is out"),
                new("dinner is served"),
                same(".")
            ]
        );
        // A real stretch of unchanged text between two changes stays.
        assert_eq!(
            changes("the loud ones can have the big room", "the talkers can have the long table"),
            [
                same("the "),
                gone("loud ones"),
                new("talkers"),
                same(" can have the "),
                gone("big room"),
                new("long table")
            ]
        );
        assert_eq!(
            changes("a big room, and more", "a long table, and more"),
            [same("a "), gone("big room"), new("long table"), same(", and more")]
        );
    }

    /// Text written without spaces changes character by character.
    #[test]
    fn unspaced_scripts_change_by_character() {
        assert_eq!(changes("我喜欢猫", "我喜欢狗"), [same("我喜欢"), gone("猫"), new("狗")]);
    }

    #[test]
    fn additions_and_removals_alone() {
        assert_eq!(
            changes("keep this line", "keep this new line"),
            [same("keep this "), new("new "), same("line")]
        );
        assert_eq!(
            changes("keep this old line", "keep this line"),
            [same("keep this "), gone("old "), same("line")]
        );
        assert_eq!(changes("", "all new"), [new("all new")]);
        assert_eq!(changes("all gone", ""), [gone("all gone")]);
        assert_eq!(changes("same", "same"), [same("same")]);
    }

    /// With little left in common, the old text and the new read better whole.
    #[test]
    fn a_rewrite_is_the_old_text_then_the_new() {
        let old = "Meet at the cafe on the corner at noon.";
        let rewritten = "Call me whenever you land; I will be up late.";
        assert_eq!(changes(old, rewritten), [gone(old), new(rewritten)]);
    }

    /// Text taken out of a sentence or a list could be marked a word or two
    /// off and spell the same two texts. It is marked where it reads whole.
    #[test]
    fn a_change_alone_falls_on_a_sentence_or_a_line() {
        assert_eq!(
            changes("A. The cat sat. The dog ran. B", "A. The dog ran. B"),
            [same("A."), gone(" The cat sat."), same(" The dog ran. B")]
        );
        assert_eq!(
            changes("Bring wine, bring bread, bring cheese.", "Bring wine, bring cheese."),
            [same("Bring wine,"), gone(" bring bread,"), same(" bring cheese.")]
        );
        assert_eq!(
            changes("the cat and the dog", "the dog"),
            [gone("the cat and "), same("the dog")]
        );
        assert_eq!(
            changes("1. six\n2. thirty\n\nThe end.", "1. six\n2. thirty\n3. many\n\nThe end."),
            [same("1. six\n2. thirty\n"), new("3. many\n"), same("\nThe end.")]
        );
    }

    /// A line that did not change holds its place: the changes on either
    /// side of a heading stay two changes, however little the heading is.
    #[test]
    fn an_unchanged_line_keeps_changes_apart() {
        assert_eq!(
            changes(
                "Keep the red door shut.\n\n## Coats\n\nLeave them out.",
                "Keep the blue gate open.\n\n## Coats\n\nHang them up."
            ),
            [
                same("Keep the "),
                gone("red door shut"),
                new("blue gate open"),
                same(".\n\n## Coats\n\n"),
                gone("Leave"),
                new("Hang"),
                same(" them "),
                gone("out"),
                new("up"),
                same(".")
            ]
        );
    }

    /// Lines that changed side by side keep their own marks: a change that
    /// took in the line break between them would read as two lines mixed.
    #[test]
    fn neighboring_lines_change_apart() {
        assert_eq!(
            changes(
                "- delicious red x\n- fluffy white bread",
                "- crisp green x\n- dense brown bread"
            ),
            [
                same("- "),
                gone("delicious red"),
                new("crisp green"),
                same(" x\n- "),
                gone("fluffy white"),
                new("dense brown"),
                same(" bread")
            ]
        );
    }

    #[test]
    fn a_removed_block_goes_whole() {
        let old =
            "## Music\n\nKeep it low.\n\n## Parking\n\nUse the lot.\n\n## Coats\n\nOn the bed.";
        let new = "## Music\n\nKeep it low.\n\n## Coats\n\nOn the bed.";
        assert_eq!(
            changes(old, new),
            [
                same("## Music\n\nKeep it low.\n\n"),
                gone("## Parking\n\nUse the lot.\n\n"),
                same("## Coats\n\nOn the bed.")
            ]
        );
    }

    /// One block rewritten among others is that block's old text, then its
    /// new; the blocks around it stand.
    #[test]
    fn a_rewritten_block_leaves_the_rest() {
        let old = "## Plan\n\nMeet at the cafe on the corner at noon.\n\n## After";
        let rewritten = "## Plan\n\nCall me whenever you land; I will be up late.\n\n## After";
        assert_eq!(
            changes(old, rewritten),
            [
                same("## Plan\n\n"),
                gone("Meet at the cafe on the corner at noon.\n\n"),
                new("Call me whenever you land; I will be up late.\n\n"),
                same("## After")
            ]
        );
    }

    /// A task checked off is one changed character. Its box is the mark.
    #[test]
    fn a_checked_task_marks_its_box() {
        assert_eq!(
            changes("- [ ] call dad\n- [x] call mom", "- [x] call dad\n- [ ] call mom"),
            [
                same("- "),
                gone("[ ]"),
                new("[x]"),
                same(" call dad\n- "),
                gone("[x]"),
                new("[ ]"),
                same(" call mom")
            ]
        );
    }

    /// The pieces spell both texts, whatever the edit.
    #[test]
    fn the_pieces_spell_both_texts() {
        let texts = [
            "",
            "one",
            "one two three",
            "one  two\n\nthree four.",
            "- [ ] dad\n- [x] mom\n",
            "\n\n# Title\r\n\r\none two\n\n\n- [x] dad\n- three\n\n",
            "| a | b |\n| --- | --- |\n| one | two |\n\nthree four.\n",
            "Naïve café, déjà vu — 你好 world 🙂!",
            "one two three four five six seven eight nine ten",
            "a completely different sentence about nothing in particular",
        ];
        for old in texts {
            for new in texts {
                let pieces = changes(old, new);
                let spell = |keep: fn(&Change) -> Option<&str>| -> String {
                    pieces.iter().filter_map(keep).collect()
                };
                let was = spell(|c| match c {
                    Change::Same(t) | Change::Gone(t) => Some(t),
                    Change::New(_) => None,
                });
                let is = spell(|c| match c {
                    Change::Same(t) | Change::New(t) => Some(t),
                    Change::Gone(_) => None,
                });
                assert_eq!((was.as_str(), is.as_str()), (old, new), "{pieces:?}");
            }
        }
    }

    #[test]
    fn long_unchanged_stretches_keep_only_their_edges() {
        let words = |n: usize| {
            (1..=n)
                .map(|i| format!("w{i}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let cut = in_context(
            vec![same(&words(10)), gone("x"), same(&words(10)), new("y"), same(&words(10))],
            2,
        );
        assert_eq!(
            cut,
            [same("… w9 w10"), gone("x"), same("w1 w2 … w9 w10"), new("y"), same("w1 w2 …")]
        );
        // Short stretches are left alone, and so is text with no change.
        let short = vec![same("a b c"), gone("x"), same("d e f g"), new("y"), same("h i")];
        assert_eq!(in_context(short.clone(), 2), short);
        assert_eq!(in_context(vec![same(&words(10))], 2), [same("…")]);
    }

    /// Lines are kept or left out whole where they fit; only a line too
    /// long to keep is cut inside.
    #[test]
    fn context_is_whole_lines_where_they_fit() {
        let list = "- one\n- two\n- three\n- four\n- five\n- six\n- seven\n";
        assert_eq!(
            in_context(vec![gone("x\n"), same(list), new("y\n")], 2),
            [gone("x\n"), same("- one\n- two\n…\n- six\n- seven\n"), new("y\n")]
        );
        let after_long = "the rest of a long line that runs on\n- a\n- b\n";
        assert_eq!(context(after_long, 2, true, true), "the rest …\n- a\n- b\n");
        let two_long = "alpha beta gamma delta\nepsilon zeta eta theta";
        assert_eq!(context(two_long, 1, true, true), "alpha …\n… theta");
        assert_eq!(context(list, 2, false, true), "…\n- six\n- seven\n");
        assert_eq!(context(list, 2, true, false), "- one\n- two\n…");
    }

    /// Whatever is cut, what stays is the text's own, in order.
    #[test]
    fn context_only_leaves_things_out() {
        let texts = [
            "one two three four five six seven eight nine ten",
            "- one\n- two\n- three\n- four\n- five\n- six\n- seven\n",
            "a long first line that runs on and on\n\n## Then\n\n- [ ] a\n- [x] b\n\nand a last long line to end on",
            "| a | b |\n| --- | --- |\n| one | two |\n| three | four |\n| five | six |\n",
            "\n\nNaïve café, déjà vu\n你好 world 🙂 again and again and again\n",
        ];
        for text in texts {
            for words in [0, 1, 2, 5] {
                for (after, before) in [(true, true), (true, false), (false, true)] {
                    let kept = context(text, words, after, before);
                    let mut rest = text;
                    for part in kept
                        .split('…')
                        .map(str::trim)
                        .filter(|part| !part.is_empty())
                    {
                        let at = rest
                            .find(part)
                            .unwrap_or_else(|| panic!("{part:?} of {kept:?} is not in {rest:?}"));
                        rest = &rest[at + part.len()..];
                    }
                    assert!(kept.len() <= text.len() + " …\n… ".len(), "{kept:?}");
                }
            }
        }
    }
}
