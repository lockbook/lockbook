//! The driver every remote property runs under: generated input with sync
//! results landing between frames, through a real workspace. A frame is
//! atomic, so the editor's only concurrency is what reaches it between
//! frames: newer versions of the note from other devices and saves'
//! completions, around frames of local input. The workspace's task manager
//! launches each load and save by its own rules; each read or write runs,
//! and is later recorded for the next frame to apply, as steps of their own.

use comrak::nodes::NodeValue;
use egui::{Event as Egui, Modifiers, PointerButton, Pos2};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use super::super::super::Editor;
use super::super::super::input::{Event, Location, Region};
use super::super::super::widget::utils::wrap_layout::FragmentContent;
use super::super::doc_gen::{Features, gen_doc};
use super::super::edit_props::{event_count, event_with};
use super::super::harness::TestEditor;
use super::note::{Landed, Note};
use super::reload::{Input, Latest};
use super::scroll::{self, BLOCK, BLOCKS, long_doc, pick_visible, settle, view_range};
use super::{Events, Host, IMAGE, Prop, lb};
use crate::test_utils::byte_source::ByteSource;

/// The run's choices, ahead of the bytes the note is made from.
const CHOICES: usize = 1024;
/// The bytes the opening note is made from; the rest make rewrites.
const DOC: usize = BLOCK * BLOCKS;

/// From [`Events::Held`] the note has images, one may be lifted and held
/// through local and remote edits, then dropped, and the caret starts on
/// screen; from [`Events::Scrolled`] the note is tall and scrolled down.
pub(super) fn run(prop: Prop, features: Features, events: Events, seeds: u64) {
    super::run(seeds, CHOICES + DOC + 256, |b, log| check(b, prop, &features, events, log));
}

struct Session<'a> {
    note: Note,
    prop: Prop,
    features: Features,
    events: Events,
    /// The bytes another device's rewrites of the note are made from.
    docs: ByteSource<'a>,
    remotes: usize,
    /// The buffer's seq once the last remote version landed.
    remote_seq: usize,
    /// An undo reached back across a remote version below [`Events::Undo`].
    left_tier: bool,
    /// The note's edits, for [`Prop::LatestVersion`].
    latest: Option<Latest>,
}

fn check(
    buf: &[u8], prop: Prop, features: &Features, events: Events, log: &mut dyn FnMut(String),
) -> Result<(), String> {
    let (choices, rest) = buf.split_at(buf.len().min(CHOICES));
    let (doc, docs) = rest.split_at(rest.len().min(DOC));
    let mut src = ByteSource::new(choices);
    let md = if events >= Events::Scrolled {
        long_doc(doc, features)
    } else if events >= Events::Held {
        doc_with_images(&mut ByteSource::new(doc), features)
    } else {
        gen_doc(&mut ByteSource::new(doc), features)
    };
    // a NUL makes the note binary to the workspace, which won't open it in the editor
    let md = md.replace('\0', "");
    log(format!("doc ({} bytes) {md:?}", md.len()));
    let mut note = Note::open(&md);
    if prop == Prop::OutlineFresh {
        note.editor_mut().toggle_outline();
    }
    let mut s = Session {
        note,
        prop,
        features: *features,
        events,
        docs: ByteSource::new(docs),
        latest: (prop == Prop::LatestVersion).then(|| Latest::new(&md)),
        remotes: 0,
        remote_seq: 0,
        left_tier: false,
    };
    s.frame(Input::None, log)?;
    if events >= Events::Scrolled {
        let wheel = 200.0 * (1 + src.draw(12)) as f32;
        log(format!("scroll down {wheel}pt"));
        scroll::scroll(&mut s.note, wheel);
        s.frame(Input::Some, log)?;
    }
    if events >= Events::Held {
        if let Some(at) = pick_visible(&s.note, &mut src) {
            let caret = Grapheme(at.source_range.start().0 + src.draw(at.len().max(1)));
            log(format!("caret at {caret:?}"));
            s.input(
                Event::Select {
                    region: Region::BetweenLocations {
                        start: Location::Grapheme(caret),
                        end: Location::Grapheme(caret),
                    },
                },
                log,
            )?;
        }
    }
    for _ in 0..8 * event_count(&mut src) {
        s.step(&mut src, log)?;
        if s.left_tier {
            return Ok(());
        }
    }
    if prop == Prop::OutlineFresh {
        outline_fresh(&mut s.note, log)?;
    }
    Ok(())
}

impl Session<'_> {
    fn seq(&self) -> usize {
        self.note.editor().edit.renderer.buffer.current.seq
    }

    fn text(&self) -> String {
        self.note.text().to_string()
    }

    /// A frame, then frames until the view settles. Only `input`, queued
    /// before it, may change the text; anything recorded lands first, in a
    /// frame of its own.
    fn frame(&mut self, input: Input, log: &mut dyn FnMut(String)) -> Result<(), String> {
        if matches!(input, Input::None) {
            self.lands(log)?;
        }
        self.note.frame_after(0.0, vec![]);
        settle(&mut self.note);
        self.observe(input, log)
    }

    /// Editor input, then a frame.
    fn input(&mut self, mut event: Event, log: &mut dyn FnMut(String)) -> Result<(), String> {
        self.lands(log)?;
        if let Event::Replace { text, .. } = &mut event {
            // a NUL makes the note binary to the workspace
            text.retain(|c| c != '\0');
        }
        self.note.t.push(event.clone());
        self.frame(Input::Event(&event), log)
    }

    /// One frame of pointer input.
    fn pointer(&mut self, input: Vec<Egui>, log: &mut dyn FnMut(String)) -> Result<(), String> {
        self.lands(log)?;
        self.note.frame_after(0.0, input);
        self.observe(Input::Some, log)
    }

    /// Applies what's recorded in a frame of its own, as the next frame
    /// would, and judges the view it lands under. A reload first applies
    /// input still queued, as a drop's move.
    fn lands(&mut self, log: &mut dyn FnMut(String)) -> Result<(), String> {
        if !self.note.recorded() {
            return Ok(());
        }
        let ctx = self.note.editor().edit.renderer.ctx.clone();
        self.note.editor_mut().edit.apply_queued(&ctx);
        self.observe(Input::Some, log)?;
        let view = self.view();
        self.note.frame_after(0.0, vec![]);
        settle(&mut self.note);
        self.observe(Input::None, log)?;
        self.judge(view, log)
    }

    /// Hands what frames applied to the checks, in the order applied.
    fn observe(&mut self, input: Input, log: &mut dyn FnMut(String)) -> Result<(), String> {
        let text = self.text();
        for landed in self.note.take_applied() {
            match landed {
                Landed::Load(disk) => {
                    self.remotes += 1;
                    self.remote_seq = self.seq();
                    log("load lands".into());
                    if let Some(latest) = &mut self.latest {
                        latest.landed(&disk, &text, log)?;
                    }
                }
                Landed::Saved(written) => {
                    log("save lands".into());
                    if let Some(latest) = &mut self.latest {
                        latest.saved(&written);
                    }
                }
                Landed::Launched => {
                    log("save launches".into());
                    if let Some(latest) = &mut self.latest {
                        latest.launched();
                    }
                }
            }
        }
        match &mut self.latest {
            Some(latest) => latest.framed(&text, input, log),
            None => Ok(()),
        }
    }

    fn step(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
        let saves = self.events >= Events::Saves;
        match src.bias(&[6, 3, 2, 3, 3, 2, 2, 2]) {
            0 => self.local(src, log),
            1 => self.remote(src, log),
            2 if saves => {
                self.note.save(log);
                self.frame(Input::None, log)
            }
            3 if saves => {
                self.note.run(src, log);
                Ok(())
            }
            4 if saves => self.record(src, log),
            5 if self.undoes() => {
                self.lands(log)?;
                let before = self.text();
                let undone = self.note.editor().edit.renderer.buffer.can_undo();
                self.undo(log)?;
                if self.prop == Prop::UndoRedo && undone && !self.left_tier {
                    log("redo".into());
                    self.input(Event::Redo, log)?;
                    let text = self.text();
                    if text != before {
                        return Err(format!("undo, redo gives {text:?}, not {before:?}"));
                    }
                }
                Ok(())
            }
            6 if self.events >= Events::Held => self.drag(src, log),
            _ => {
                log("frame".into());
                self.frame(Input::None, log)
            }
        }
    }

    /// Records one run read or write; a load waits for any already
    /// recorded to land, so each lands in a frame of its own.
    fn record(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
        if self.note.ran() == 0 {
            return Ok(());
        }
        let i = src.draw(self.note.ran());
        if self.note.ran_load(i) && self.note.load_recorded() {
            self.lands(log)?;
        }
        self.note.record_at(i, log);
        Ok(())
    }

    /// Runs and records everything in flight, framing until none is left.
    fn settle_tasks(&mut self, log: &mut dyn FnMut(String)) -> Result<(), String> {
        let mut src = ByteSource::new(&[]);
        for _ in 0..50 {
            if self.note.idle() {
                return Ok(());
            }
            while self.note.held() > 0 {
                self.note.run(&mut src, log);
            }
            while self.note.ran() > 0 {
                self.record(&mut src, log)?;
            }
            self.frame(Input::None, log)?;
        }
        Err("the note's loads and saves never settled".into())
    }

    /// Below [`Events::Undo`], only local edits since the last remote
    /// version are undone.
    fn undoes(&self) -> bool {
        self.events >= Events::Undo || self.remotes == 0 || self.seq() > self.remote_seq
    }

    /// An undo, then a frame. Undo groups typing, so one can still reach
    /// across a remote version; the run then stops, as it left its tier.
    fn undo(&mut self, log: &mut dyn FnMut(String)) -> Result<(), String> {
        log("undo".into());
        self.input(Event::Undo, log)?;
        let across = self.remotes > 0 && self.seq() < self.remote_seq;
        if self.events < Events::Undo && across {
            log("the undo reached back across a remote version".into());
            self.left_tier = true;
        }
        Ok(())
    }

    /// One generated input event, then a frame.
    fn local(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
        // input is made against the text the frame before it shows
        self.lands(log)?;
        let event = event_with(src, self.note.editor(), &self.features);
        if matches!(event, Event::Undo) {
            return if self.undoes() { self.undo(log) } else { self.frame(Input::None, log) };
        }
        log(format!("local {event:?}"));
        self.input(event, log)
    }

    /// Other devices' versions of the note, one or more, written as a sync
    /// writes them, with or without frames between; the workspace loads
    /// them by its own rules. Below [`Events::Typing`] they land on a
    /// just-saved note before more input.
    fn remote(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
        if self.events < Events::Reading {
            return Ok(());
        }
        let reading = self.events < Events::Typing;
        if reading {
            self.note.save(log);
            self.settle_tasks(log)?;
            if !self.note.idle() || self.text() != self.note.disk() {
                return Ok(());
            }
        }
        let versions = 1 + src.bias(&[4, 2, 1]);
        let framed = src.bias(&[3, 1]) == 0;
        for _ in 0..versions {
            let version = self.version(src, log);
            self.note.remote(&version);
            if framed {
                self.frame(Input::None, log)?;
            }
        }
        if reading {
            self.settle_tasks(log)?;
        }
        Ok(())
    }

    /// The view before versions land, for the properties that judge it.
    fn view(&mut self) -> Option<scroll::Before> {
        matches!(self.prop, Prop::TextStays | Prop::CaretStays)
            .then(|| scroll::Before::new(&mut self.note, self.prop))
            .flatten()
    }

    fn judge(
        &mut self, view: Option<scroll::Before>, log: &mut dyn FnMut(String),
    ) -> Result<(), String> {
        let scrolled = self.events >= Events::Scrolled;
        view.map_or(Ok(()), |view| view.judge(&mut self.note, self.prop, scrolled, log))
    }

    /// Another device's next version: an edit at a spot of the note as
    /// shown, carried onto the version on disk without the note's unsaved
    /// edits, or the version on disk reworked whole.
    fn version(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> String {
        let text = self.text();
        let disk = self.note.disk();
        let bounds: Vec<usize> = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([text.len()])
            .collect();
        let last = bounds.len() - 1;
        // from Scrolled the spot may be wholly above or below the view
        let view = if self.events >= Events::Scrolled { view_range(&self.note) } else { None };
        let (lo, hi) = match (view, src.bias(&[2, 1, 1])) {
            (Some((lo, _)), 1) => (0, lo.0.min(last)),
            (Some((_, hi)), 2) => (hi.0.min(last), last),
            _ => (0, last),
        };
        let a = lo + src.draw(hi - lo + 1);
        let b = (a + src.draw(40)).min(hi);
        let marker = format!("⟦{}⟧", self.remotes + 1);
        let lines: Vec<&str> = disk.split_inclusive('\n').collect();
        let next = match src.bias(&[4, 2, 2, 2, 2, 1, 1]) {
            kind @ 0..=2 => {
                let (end, with) = match kind {
                    0 => {
                        let image = format!("\n\n![a]({IMAGE})\n\n");
                        let para = "\n\nanother device wrote this paragraph\n\n";
                        let inserts = [&marker, "foo ", "\n", " and some words", "- [ ] ", para];
                        let with = inserts.get(src.draw(7)).map_or(image, |s| s.to_string());
                        (a, with)
                    }
                    1 => (b, String::new()),
                    _ => (b, marker),
                };
                log(format!(
                    "remote replaces {:?} at {a} with {with:?}",
                    &text[bounds[a]..bounds[end]]
                ));
                let mut next = text.clone();
                next.replace_range(bounds[a]..bounds[end], &with);
                if text == disk { next } else { Buffer::from(text.as_str()).merge(next, disk) }
            }
            3 if !lines.is_empty() => {
                log("remote repeats a line".into());
                let mut out = lines.clone();
                out.insert(src.draw(lines.len() + 1), lines[src.draw(lines.len())]);
                out.concat()
            }
            4 if lines.len() > 1 => {
                log("remote swaps two lines".into());
                let mut out = lines.clone();
                out.swap(src.draw(lines.len()), src.draw(lines.len()));
                out.concat()
            }
            5 => {
                log("remote rewrites the note".into());
                gen_doc(&mut self.docs, &self.features).replace('\0', "")
            }
            _ => {
                log("remote rewrites the note, keeping its words in a new order".into());
                let mut words: Vec<&str> = disk.split_word_bounds().collect();
                let by = src.draw(words.len().max(1)).min(words.len());
                words.rotate_left(by);
                words.concat()
            }
        };
        log(format!("disk becomes {next:?}"));
        next
    }

    /// A version from another device inserting a marker outside any image,
    /// written as the drop lets go, so it lands before the next frame moves
    /// the image. `None` unless nothing is in flight or unsaved, since a
    /// merge may drop an insert into an edit, or if no spot keeps it whole.
    fn remote_marker(
        &mut self, src: &mut ByteSource, log: &mut dyn FnMut(String),
    ) -> Option<String> {
        let disk = self.note.disk();
        if !self.note.idle() || self.text() != disk {
            return None;
        }
        let marker = format!("⟦{}⟧", self.remotes + 1);
        let images = image_ranges(&disk);
        // the marker's ends can't match the text beside it, or the diff finds
        // the insert shifted
        let (first, last) = (marker.chars().next(), marker.chars().last());
        let bounds: Vec<usize> = disk
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([disk.len()])
            .filter(|&i| !images.iter().any(|r| r.start < i && i < r.end))
            .filter(|&i| disk[i..].chars().next() != first && disk[..i].chars().last() != last)
            .collect();
        if bounds.is_empty() {
            return None;
        }
        let at = bounds[src.draw(bounds.len())];
        log(format!("remote inserts {marker:?} at byte {at} as the drop lets go"));
        let mut next = disk;
        next.insert_str(at, &marker);
        self.note.remote(&next);
        Some(marker)
    }

    /// Lifts an image with the mouse, holds it through local edits, saves and
    /// remote versions, and drops it somewhere.
    fn drag(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
        let Some(from) = image_center(&self.note, src) else { return Ok(()) };
        let button = |pos, pressed| Egui::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        log(format!("drag lifts the image at {from:?}"));
        self.pointer(vec![Egui::PointerMoved(from)], log)?;
        self.pointer(vec![button(from, true)], log)?;
        self.pointer(vec![Egui::PointerMoved(from + egui::vec2(20.0, 20.0))], log)?;
        let drag = self.note.editor().edit.in_progress_image_drag.as_ref();
        let Some(grab) = drag.map(|d| d.grab_offset) else {
            return self.pointer(vec![button(from, false)], log);
        };
        let lift = egui::vec2(0.0, grab.y + self.note.editor().edit.renderer.drop_lift());
        log("drag holds".into());
        for _ in 0..src.draw(4) {
            match src.bias(&[1, 1, 1, 1, 1]) {
                0 => self.remote(src, log)?,
                1 => {
                    self.note.save(log);
                    self.frame(Input::None, log)?
                }
                2 => self.note.run(src, log),
                3 => self.record(src, log)?,
                _ => self.local(src, log)?,
            }
        }
        let segs = &self.note.editor().edit.renderer.buffer.current.segs;
        let at = Grapheme(src.draw(segs.last_cursor_position().0 + 1));
        let to = self
            .note
            .editor()
            .edit
            .cursor_line(at)
            .map(|[top, bot]| Pos2::new(top.x - 1.0, (top.y + bot.y) / 2.0))
            .unwrap_or(from);
        log(format!("drag drops at offset {at:?}"));
        self.pointer(vec![Egui::PointerMoved(to)], log)?;
        self.pointer(vec![Egui::PointerMoved(to + lift)], log)?;
        let before = self.text();
        // a version can land before the next frame moves the image
        let marker = if src.bias(&[1, 1]) == 1 { self.remote_marker(src, log) } else { None };
        self.pointer(vec![button(to + lift, false)], log)?;
        let dropped = self.note.editor().edit.in_progress_image_drag.as_ref();
        let dropped = dropped
            .filter(|d| d.dropped)
            .and_then(|d| Some((d.image, d.target?)));
        if marker.is_some() {
            self.settle_tasks(log)?;
        }
        let moved = dropped.map(|(range, to)| Event::Move { range, to });
        self.frame(moved.as_ref().map_or(Input::Some, Input::Event), log)?;
        let mut after = self.text();
        if let Some(marker) = marker {
            if after.matches(&marker).count() != 1 {
                return Err(format!(
                    "the version landing with the drop lost {marker:?}: {after:?}"
                ));
            }
            after = after.replace(&marker, "");
        }
        if after != before {
            log("the drop moved the image".into());
        }
        if self.prop == Prop::DropMovesImage {
            drop_moves_image(&before, &after, dropped)?;
        }
        Ok(())
    }
}

/// A drop moves only the image: its text lands where the drop line put it,
/// and the characters, the images and the text around them are as before.
fn drop_moves_image(
    before: &str, after: &str, dropped: Option<((Grapheme, Grapheme), Grapheme)>,
) -> Result<(), String> {
    if let Some((image, to)) = dropped.filter(|&(image, to)| !image.contains_inclusive(to)) {
        let graphemes: Vec<&str> = before.graphemes(true).collect();
        let text = graphemes[image.0.0..image.1.0].concat();
        let len = image.1.0 - image.0.0;
        let at = if to > image.1 { to.0 - len } else { to.0 };
        let landed: String = after.graphemes(true).skip(at).take(len).collect();
        if landed != text {
            return Err(format!("{text:?} dropped at {to:?} isn't there: {after:?}"));
        }
    }
    // an image leaving a table row without pipes leaves the row, emptied, as `|  |`
    let row = regex::Regex::new(r"(?m)^\| *\|$").unwrap();
    let emptied = |t: &str| row.replace_all(t, "").into_owned();
    let sorted = |t: &str| {
        let mut c: Vec<char> = emptied(t).chars().collect();
        c.sort_unstable();
        c
    };
    let images = |t: &str| {
        let mut i: Vec<&str> = image_ranges(t).into_iter().map(|r| &t[r]).collect();
        i.sort_unstable();
        i.join("\n")
    };
    // the text around the images, in order
    let rest = |t: &str| -> String {
        let ranges = image_ranges(t);
        let around = t
            .char_indices()
            .filter(|(i, _)| !ranges.iter().any(|r| r.contains(i)))
            .map(|(_, c)| c);
        emptied(&around.collect::<String>())
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    };
    if sorted(before) != sorted(after)
        || images(before) != images(after)
        || rest(before) != rest(after)
    {
        return Err(format!("the drop turned {before:?} into {after:?}"));
    }
    Ok(())
}

/// The byte ranges of the images in `md` as the editor parses it.
fn image_ranges(md: &str) -> Vec<std::ops::Range<usize>> {
    let mut ws = TestEditor::with_lb(lb(), md);
    let arena = super::super::super::Arena::new();
    let r = &mut ws.editor.edit.renderer;
    let root = r.reparse(&arena);
    let segs = &r.buffer.current.segs;
    root.descendants()
        .filter(|n| matches!(n.data.borrow().value, NodeValue::Image(_)))
        .map(|n| {
            let (a, b) = segs.range_to_byte(r.node_range(n));
            a.0..b.0
        })
        .collect()
}

/// The center of one of the images drawn in view, if any.
fn image_center(ws: &impl Host, src: &mut ByteSource) -> Option<Pos2> {
    let view = ws.view();
    let images: Vec<Pos2> = ws
        .editor()
        .edit
        .renderer
        .fragments
        .iter()
        .filter(|f| matches!(f.content, FragmentContent::Embed { .. }))
        .map(|f| f.rect.center())
        .filter(|c| (view.top()..view.bottom()).contains(&c.y))
        .collect();
    (!images.is_empty()).then(|| images[src.draw(images.len())])
}

/// A generated note with up to two images as paragraphs of their own.
fn doc_with_images(src: &mut ByteSource, features: &Features) -> String {
    let doc = gen_doc(src, features);
    let mut blocks: Vec<String> = doc.split("\n\n").map(str::to_string).collect();
    for _ in 0..src.draw(3) {
        let at = src.draw(blocks.len() + 1);
        blocks.insert(at, format!("![a]({IMAGE})"));
    }
    blocks.join("\n\n")
}

/// The outline as kept through every change equals the outline of a fresh
/// open of the final text.
fn outline_fresh(ws: &mut impl Host, log: &mut dyn FnMut(String)) -> Result<(), String> {
    ws.frame_after(0.0, vec![]);
    let rows = |editor: &Editor| {
        editor
            .outline_rows
            .as_ref()
            .map(|(_, rows)| {
                rows.iter()
                    .map(|r| (r.level, r.text.clone(), r.start, r.span))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let text = ws.text().to_string();
    let mut fresh = TestEditor::with_lb(lb(), &text);
    fresh.editor.toggle_outline();
    fresh.enter_frame();
    let (kept, opened) = (rows(ws.editor()), rows(&fresh.editor));
    log(format!("outline kept {kept:?}, fresh {opened:?}"));
    if kept != opened {
        return Err(format!("outline kept {kept:?}, a fresh open lists {opened:?} for {text:?}"));
    }
    Ok(())
}
