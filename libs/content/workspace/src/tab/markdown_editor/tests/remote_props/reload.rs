//! The note shows the latest version. Each new version on disk reaches the
//! editor through `Editor::reload`; after it the note shows that version
//! merged with the note's edits one by one: edits apart from each other both
//! land, the note's first where both insert, and where they overlap the note
//! keeps its own. A frame changes the text only by the input in it.

use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::input::{Event, Location, Region};
use unicode_segmentation::UnicodeSegmentation as _;

/// What could have changed the text in a frame.
pub(super) enum Input<'a> {
    None,
    /// Input the case doesn't spell out, as a drag.
    Some,
    Event(&'a Event),
}

/// The note's edits as each alignment of them it could have made, and the
/// text they show.
pub(super) struct Latest {
    models: Vec<Model>,
    shown: String,
}

impl Latest {
    pub(super) fn new(disk: &str) -> Self {
        Self { models: vec![Model::new(disk)], shown: disk.into() }
    }

    /// A version landed: the note shows it merged with one alignment.
    pub(super) fn landed(
        &mut self, disk: &str, shown: &str, log: &mut dyn FnMut(String),
    ) -> Result<(), String> {
        let merged: Vec<Model> = self.models.drain(..).map(|m| m.reload(disk)).collect();
        let wants: Vec<String> = merged.iter().map(Model::shown).collect();
        if !wants.iter().any(|w| w == shown) {
            return fail(shown, &wants, log);
        }
        self.models = merged
            .into_iter()
            .filter(|m| m.shown() == shown)
            .take(MODELS)
            .collect();
        self.shown = shown.into();
        Ok(())
    }

    /// A frame passed; only `input` may have changed the text.
    pub(super) fn framed(
        &mut self, shown: &str, input: Input, log: &mut dyn FnMut(String),
    ) -> Result<(), String> {
        if shown == self.shown {
            return Ok(());
        }
        let models = match input {
            Input::None => return fail(shown, std::slice::from_ref(&self.shown), log),
            Input::Event(event) => self.said(event, shown),
            Input::Some => vec![],
        };
        self.models =
            if models.is_empty() { inferred(&self.models, &self.shown, shown) } else { models };
        if self.models.is_empty() {
            return Err(format!("no alignment of the note's edits makes {shown:?}"));
        }
        self.shown = shown.into();
        Ok(())
    }

    /// The models where `event` made `shown` as it says: text written at
    /// the spot it names, or an undo stepping back through the note's edits.
    fn said(&self, event: &Event, shown: &str) -> Vec<Model> {
        let (region, text) = match event {
            Event::Undo => {
                let undone = self.models.iter().filter_map(|m| m.undone(shown));
                return distinct(undone, MODELS);
            }
            Event::Move { range, to } => return self.moved(*range, *to, shown),
            Event::Replace { region, text, .. } => (region, text.as_str()),
            Event::Delete { region } => (region, ""),
            _ => return vec![],
        };
        let Region::BetweenLocations { start: Location::Grapheme(a), end: Location::Grapheme(b) } =
            *region
        else {
            return vec![];
        };
        let segs = lb_rs::model::text::unicode_segs::calc(&self.shown);
        let (s, e) = segs.range_to_byte((a.min(b), a.max(b)));
        let edit = Edit { range: (s.0, e.0), text: text.into() };
        let mut after = self.shown.clone();
        edit.apply(&mut after);
        let mut models = self.models.clone();
        for m in &mut models {
            m.edits.push(edit.clone());
        }
        if after == shown { models } else { vec![] }
    }

    /// The models where a drop moved `range` to `to` to make `shown`: its
    /// text taken out, which in a table rewrites the row around it, and
    /// written at `to`, in one frame. The buffer orders a frame's edits by
    /// where they start and joins an edit onto one ending where it starts.
    fn moved(&self, range: (Grapheme, Grapheme), to: Grapheme, shown: &str) -> Vec<Model> {
        let segs = lb_rs::model::text::unicode_segs::calc(&self.shown);
        let (a, b) = segs.range_to_byte(range);
        let to = segs.offset_to_byte(to).0;
        let Some(text) = self.shown.get(a.0..b.0).filter(|t| !t.is_empty()) else {
            return vec![];
        };
        // the edits in the order the buffer keeps them, given what taking the
        // text out wrote at `taken`
        let ordered = |taken: Edit| {
            let (d0, d1) = taken.range;
            let written = Edit { range: (to, to), text: text.into() };
            if to < d0 {
                let shift = text.len();
                vec![written, Edit { range: (d0 + shift, d1 + shift), ..taken }]
            } else if to > d1 {
                let at = to - d1 + d0 + taken.text.len();
                vec![taken, Edit { range: (at, at), ..written }]
            } else {
                vec![Edit { range: (d0, d1), text: format!("{}{text}", taken.text) }]
            }
        };
        let with = |m: &Model, edits: Vec<Edit>| {
            let mut m = m.clone();
            m.edits.extend(edits);
            (m.shown() == shown).then_some(m)
        };
        let exact = Edit { range: (a.0, b.0), text: String::new() };
        let models: Vec<Model> = self
            .models
            .iter()
            .filter_map(|m| with(m, ordered(exact.clone())))
            .collect();
        if !models.is_empty() {
            return models;
        }
        // in a table: the row as rewritten without the text, then the text
        // written at the drop
        let mut models = vec![];
        for (at, _) in shown.match_indices(text) {
            let mut taken = shown.to_string();
            taken.replace_range(at..at + text.len(), "");
            for mut m in self.models.iter().flat_map(|m| m.edited(&taken)) {
                let taken = m.edits.pop().unwrap();
                models.extend(with(&m, ordered(taken)));
            }
        }
        distinct(models, MODELS)
    }
    /// A save launched with the edits so far.
    pub(super) fn launched(&mut self) {
        for m in &mut self.models {
            m.flushed = m.edits.len();
        }
    }

    /// The launched save landed with `text`, which its edits made.
    pub(super) fn saved(&mut self, text: &str) {
        for m in &mut self.models {
            m.edits.drain(..m.flushed);
            m.flushed = 0;
            m.landed = text.into();
        }
    }
}

/// `models`, showing `from`, for each way input could have made `to`, as the
/// texts it steps through, taking turns at the alignments followed.
fn inferred(models: &[Model], from: &str, to: &str) -> Vec<Model> {
    let mut ways: Vec<std::vec::IntoIter<Model>> = ways(from, to)
        .iter()
        .map(|steps| {
            let mut models = models.to_vec();
            for step in steps {
                models = distinct(models.iter().flat_map(|m| m.edited(step)), MODELS);
            }
            models.into_iter()
        })
        .collect();
    let turns = std::iter::from_fn(|| {
        let turn: Vec<Model> = ways.iter_mut().filter_map(Iterator::next).collect();
        (!turn.is_empty()).then_some(turn)
    });
    distinct(turns.flatten(), MODELS)
}

/// The texts input could have stepped through from `before` to `after`:
/// one edit, as most input makes; one per piece a diff finds, as an undo
/// putting back several; and for a move, as a drop makes, the moved text
/// deleted then inserted, whichever side of the text it passed it was.
fn ways(before: &str, after: &str) -> Vec<Vec<String>> {
    let mut ways = vec![vec![after.to_string()]];
    let segs = lb_rs::model::text::unicode_segs::calc(before);
    let pieces = lb_rs::model::text::diff(before, after);
    if pieces.len() > 1 {
        let mut step = before.to_string();
        let steps = pieces.into_iter().rev().map(|r| {
            let (a, b) = segs.range_to_byte(r.range);
            step.replace_range(a.0..b.0, &r.text);
            step.clone()
        });
        ways.push(steps.collect());
    }
    if before.len() != after.len() {
        return ways;
    }
    let len = before.len();
    let prefix = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = before
        .bytes()
        .rev()
        .zip(after.bytes().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(len - prefix);
    // the moved text and the text it passed may share their first or last
    // characters, which the common prefix or suffix then takes
    let trims = (0..=prefix)
        .map(|p| (p, suffix))
        .chain((0..suffix).map(|s| (prefix, s)));
    for (p, s) in trims {
        let (Some(was), Some(now)) = (before.get(p..len - s), after.get(p..len - s)) else {
            continue;
        };
        // `was` is `a` then `b`, and `now` is `b` then `a`
        let twice = format!("{was}{was}");
        for (k, _) in twice
            .match_indices(now)
            .filter(|&(k, _)| 0 < k && k < was.len())
        {
            if !was.is_char_boundary(k) {
                continue;
            }
            let (a, b) = was.split_at(k);
            let (head, tail) = (&before[..p], &before[len - s..]);
            for step in [format!("{head}{b}{tail}"), format!("{head}{a}{tail}")] {
                let way = vec![step, after.to_string()];
                if !ways.contains(&way) {
                    ways.push(way);
                }
            }
        }
    }
    ways
}

/// The most alignments of the note's edits followed at once: Held seed 449
/// follows 744.
const MODELS: usize = 1024;

/// The first `n` of `models` with edits apart from the ones before.
fn distinct(models: impl IntoIterator<Item = Model>, n: usize) -> Vec<Model> {
    let mut kept: Vec<Model> = vec![];
    for m in models.into_iter() {
        if kept.len() == n {
            break;
        }
        if !kept.iter().any(|k| k.edits == m.edits) {
            kept.push(m);
        }
    }
    kept
}

fn fail(shown: &str, wants: &[String], log: &mut dyn FnMut(String)) -> Result<(), String> {
    log(format!("shows {shown:?}"));
    Err(format!("the note shows {shown:?}, it should show one of {wants:?}"))
}

/// What the note should show: the version it last loaded and the note's
/// edits since, in order. A new version merges edit by edit, as the buffer
/// keeps them: an edit and one right after it that puts the text back drop
/// out, and each edit splits into the pieces a diff of what it replaced and what it wrote finds.
/// In order, each piece then takes over the remote edits it overlaps, the
/// text under them staying the note's own; a remote edit apart from it moves
/// past it, one at a local insert's spot landing after it and an insert at a
/// replaced range's start before it. Where the text alone can't say which
/// edit the note made, a model follows each.
#[derive(Clone)]
struct Model {
    landed: String,
    edits: Vec<Edit>,
    /// The edits a save in flight writes.
    flushed: usize,
}

/// A byte range of the text the edits before it leave, and what it writes there.
#[derive(Clone, Debug, PartialEq)]
struct Edit {
    range: (usize, usize),
    text: String,
}

impl Edit {
    fn apply(&self, text: &mut String) {
        text.replace_range(self.range.0..self.range.1, &self.text);
    }

    fn is_empty(&self) -> bool {
        self.range.0 == self.range.1
    }

    /// The edit to `text` widened to whole graphemes of the text after it,
    /// as the buffer widens it: a mark it writes fuses onto the one before.
    fn whole_graphemes(&self, text: &str) -> Edit {
        let bounds = |t: &str| -> Vec<usize> {
            t.grapheme_indices(true)
                .map(|(i, _)| i)
                .chain([t.len()])
                .collect()
        };
        let ((a, b), end) = (self.range, self.range.0 + self.text.len());
        let mut after = text.to_string();
        self.apply(&mut after);
        let (was, now) = (bounds(text), bounds(&after));
        let lo = *now.iter().rev().find(|&&i| i <= a).unwrap();
        let hi = *now.iter().find(|&&i| i >= end).unwrap();
        let start = (*was.iter().rev().find(|&&i| i <= lo).unwrap()).min(a);
        let stop = (*was.iter().find(|&&i| i >= b + hi - end).unwrap()).max(b);
        Edit { range: (start, stop), text: after[start..stop + end - b].to_string() }
    }
}

/// Where `at` lands once `range` is replaced with `len` bytes: past them if
/// `advance`, for a spot within the range or at its ends.
fn shift(at: usize, range: (usize, usize), len: usize, advance: bool) -> usize {
    if at < range.0 {
        at
    } else if at > range.1 {
        at + len - (range.1 - range.0)
    } else if advance {
        range.0 + len
    } else {
        range.0
    }
}

/// `text` with `edits`, ordered and apart, applied, and where each wrote.
fn merged(text: &str, edits: &[Edit]) -> (String, Vec<(usize, usize)>) {
    let mut out = text.to_string();
    let mut grown = 0isize;
    let mut written = vec![];
    for edit in edits {
        let at = edit.range.0.checked_add_signed(grown).unwrap();
        written.push((at, at + edit.text.len()));
        grown += edit.text.len() as isize - (edit.range.1 - edit.range.0) as isize;
    }
    for edit in edits.iter().rev() {
        edit.apply(&mut out);
    }
    (out, written)
}

/// Whether a local edit and a remote one conflict: they share text, or one
/// inserts strictly inside the other.
fn overlap((a0, a1): (usize, usize), (b0, b1): (usize, usize)) -> bool {
    a0 < b1 && b0 < a1
}

impl Model {
    fn new(disk: &str) -> Self {
        Self { landed: disk.into(), edits: vec![], flushed: 0 }
    }

    fn shown(&self) -> String {
        let mut text = self.landed.clone();
        for edit in &self.edits {
            edit.apply(&mut text);
        }
        text
    }

    /// The models the shown text becomes `after` in by one edit: one per spot
    /// the edit could be at, where it slides along text equal to what it
    /// writes.
    fn edited(&self, after: &str) -> Vec<Model> {
        let before = self.shown();
        if before == after {
            return vec![self.clone()];
        }
        let prefix = before
            .bytes()
            .zip(after.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        let suffix = before
            .bytes()
            .rev()
            .zip(after.bytes().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let bounds: Vec<usize> = before
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([before.len()])
            .collect();
        // the edit's sizes at its latest spot, widened to whole graphemes as the buffer does
        let mut start = prefix.min(before.len()).min(after.len());
        while !bounds.contains(&start) {
            start -= 1;
        }
        let mut tail = suffix.min(before.len() - start).min(after.len() - start);
        while !bounds.contains(&(before.len() - tail)) {
            tail -= 1;
        }
        let (gone, written) = (before.len() - tail - start, after.len() - tail - start);
        let earliest = before.len().saturating_sub(gone + suffix);
        bounds
            .iter()
            .filter(|&&s| earliest <= s && s <= start && bounds.contains(&(s + gone)))
            .map(|&s| {
                let mut m = self.clone();
                m.edits
                    .push(Edit { range: (s, s + gone), text: after[s..s + written].into() });
                m
            })
            .collect()
    }

    /// The model with its last edits reverted, last first, as an undo steps
    /// back through them, if that shows `after`.
    fn undone(&self, after: &str) -> Option<Model> {
        let mut texts = vec![self.landed.clone()];
        for edit in &self.edits {
            let mut text = texts.last().unwrap().clone();
            edit.apply(&mut text);
            texts.push(text);
        }
        let mut m = self.clone();
        for (edit, before) in self.edits.iter().zip(&texts).rev() {
            let (a, b) = edit.range;
            m.edits
                .push(Edit { range: (a, a + edit.text.len()), text: before[a..b].into() });
            if before == after {
                return Some(m);
            }
        }
        None
    }

    /// Drops each edit along with the one right after it that leaves the text
    /// as it was before it, as an undo or a backspace does, then splits each
    /// edit into what it changes.
    fn tidy(&mut self) {
        // each kept edit with the text before it
        let mut text = self.landed.clone();
        let mut kept: Vec<(Edit, String)> = vec![];
        for edit in std::mem::take(&mut self.edits) {
            let before = text.clone();
            edit.apply(&mut text);
            if kept.last().is_some_and(|(_, was)| *was == text) {
                kept.pop();
            } else {
                kept.push((edit, before));
            }
        }
        // the pieces of each edit, last first so each range is where the diff put it
        let mut text = self.landed.clone();
        for (edit, _) in kept {
            let replaced = text[edit.range.0..edit.range.1].to_string();
            let segs = lb_rs::model::text::unicode_segs::calc(&replaced);
            for r in lb_rs::model::text::diff(&replaced, &edit.text)
                .into_iter()
                .rev()
            {
                let (a, b) = segs.range_to_byte(r.range);
                let piece = Edit { range: (edit.range.0 + a.0, edit.range.0 + b.0), text: r.text };
                piece.apply(&mut text);
                self.edits.push(piece);
            }
        }
    }

    /// The model with `disk` merged in.
    fn reload(mut self, disk: &str) -> Self {
        self.tidy();
        let segs = lb_rs::model::text::unicode_segs::calc(&self.landed);
        // the remote edits on `local`, the text before the next local edit;
        // `theirs` is `local` with them applied, and each wrote `written` there
        let mut remote: Vec<Edit> = lb_rs::model::text::diff(&self.landed, disk)
            .into_iter()
            .map(|r| {
                let (s, e) = segs.range_to_byte(r.range);
                Edit { range: (s.0, e.0), text: r.text }
            })
            .collect();
        let mut local = self.landed.clone();
        let (mut theirs, mut written) = merged(&local, &remote);
        for edit in std::mem::take(&mut self.edits) {
            let edit = edit.whole_graphemes(&local);
            let dropped: Vec<bool> = remote
                .iter()
                .map(|r| overlap(edit.range, r.range))
                .collect();
            // the region the edit takes over: its range and the remote edits it drops
            let region = remote
                .iter()
                .zip(&dropped)
                .filter(|(_, d)| **d)
                .fold(edit.range, |(a, b), (r, _)| (a.min(r.range.0), b.max(r.range.1)));
            // a kept remote edit lies before the region if it starts before it,
            // or inserts where a replaced region starts
            let before: Vec<Option<bool>> = remote
                .iter()
                .zip(&dropped)
                .map(|(r, d)| {
                    let (s, replaced) = (r.range.0, region.0 < region.1);
                    (!d).then_some(s < region.0 || (s == region.0 && r.is_empty() && replaced))
                })
                .collect();

            // the kept ones move past the edit, an insert at a replaced range's start before it
            let len = edit.text.len();
            let kept: Vec<Edit> = remote
                .iter()
                .zip(&dropped)
                .filter(|(_, d)| !**d)
                .map(|(r, _)| {
                    let at_start = !edit.is_empty() && r.range.0 == edit.range.0;
                    let insert = r.is_empty();
                    let start = shift(r.range.0, edit.range, len, !(insert && at_start));
                    let end = shift(r.range.1, edit.range, len, insert && !at_start);
                    Edit { range: (start, end), text: r.text.clone() }
                })
                .collect();
            let local_end = shift(region.1, edit.range, len, true);
            edit.apply(&mut local);

            // in `theirs` the region also covers what the dropped edits wrote
            let to_theirs = |mut at: usize, past_dropped: bool| {
                for ((r, w), before) in remote.iter().zip(&written).zip(&before) {
                    let range = (w.0, w.0 + r.range.1 - r.range.0);
                    at = shift(at, range, w.1 - w.0, before.unwrap_or(past_dropped));
                }
                at
            };
            let rebased = Edit {
                range: (to_theirs(region.0, false), to_theirs(region.1, true)),
                text: local[region.0..local_end].into(),
            };
            rebased.apply(&mut theirs);
            let (range, len) = (rebased.range, rebased.text.len());
            written = written
                .iter()
                .zip(&before)
                .filter_map(|(w, before)| {
                    let past = !(*before)?;
                    Some((shift(w.0, range, len, past), shift(w.1, range, len, past)))
                })
                .collect();
            remote = kept;
            self.edits.push(rebased);
            debug_assert_eq!(
                merged(&local, &remote),
                (theirs.clone(), written.clone()),
                "the kept remote edits stay where they wrote"
            );
        }
        self.landed = disk.into();
        self
    }
}

/// The shortest cases behind the row, against the buffer alone.
#[test]
fn reload_cases() {
    use lb_rs::model::text::buffer::Buffer;
    use lb_rs::model::text::operation_types::{Operation, Replace};

    // reading only: one new version
    let mut b = Buffer::from("foo 👋 foo\n");
    b.reload("foo 👋foo  foo\n".into());
    b.update();
    assert_eq!(b.current.text, "foo 👋foo  foo\n");

    // typing one letter between versions
    let mut b = Buffer::from("foo foo foo foo foo\n");
    for v in ["foo foo\n", "foo foo foo\n"] {
        b.reload(v.into());
        b.update();
    }
    b.queue(
        vec![Operation::Replace(Replace { range: (Grapheme(0), Grapheme(0)), text: "Ж".into() })],
        std::time::Duration::ZERO,
    );
    b.update();
    for v in ["foo foo foo foo\n", "foo foo foo foo foo\n"] {
        b.reload(v.into());
        b.update();
    }
    assert_eq!(b.current.text, "Жfoo foo foo foo foo\n");
}
