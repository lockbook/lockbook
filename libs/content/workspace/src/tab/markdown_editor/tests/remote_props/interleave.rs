//! The driver every remote property runs under: generated input with sync
//! results landing between frames. A frame is atomic, so the editor's only
//! concurrency is which of them reach it between frames: newer versions of
//! the note from a sync ([`Editor::reload`]) or a save's completion
//! ([`Editor::saved`]), around frames of local input. The task manager runs
//! one load or save of a file at a time, may collapse several loads into the
//! latest, and applies a finished save before the next load starts, so a
//! remote version only lands while no save is in flight.
//!
//! [`Editor::reload`]: super::super::Editor::reload
//! [`Editor::saved`]: super::super::Editor::saved

use comrak::nodes::NodeValue;
use egui::{Event as Egui, Modifiers, PointerButton, Pos2};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use super::super::super::input::{Event, Location, Region};
use super::super::super::widget::utils::wrap_layout::FragmentContent;
use super::super::doc_gen::{Features, gen_doc};
use super::super::edit_props::{event_count, event_with};
use super::super::harness::TestEditor;
use super::reload::Latest;
use super::scroll::{self, BLOCK, BLOCKS, VIEW, long_doc, pick_visible, settle, view_range};
use super::{Events, IMAGE, Prop, lb};
use crate::test_utils::byte_source::ByteSource;

/// The run's choices, ahead of the bytes the note is made from.
const CHOICES: usize = 256;
/// The bytes the opening note is made from; the rest make rewrites.
const DOC: usize = BLOCK * BLOCKS;

/// From [`Events::Held`] the note has images, one may be lifted and held
/// through local and remote edits, then dropped, and the caret starts on
/// screen; from [`Events::Scrolled`] the note is tall and scrolled down.
pub(super) fn run(prop: Prop, features: Features, events: Events, seeds: u64) {
    super::run(seeds, CHOICES + DOC + 256, |b, log| check(b, prop, &features, events, log));
}

/// The note as both sides last agreed on it, and the save in flight.
struct Session<'a> {
    ws: TestEditor,
    prop: Prop,
    features: Features,
    events: Events,
    /// The bytes another device's rewrites of the note are made from.
    docs: ByteSource<'a>,
    /// The seq and text a launched save will record.
    save: Option<(usize, String)>,
    /// The note on disk, which a remote edit starts from.
    disk: String,
    saves: u8,
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
    log(format!("doc ({} bytes) {md:?}", md.len()));
    let (mut ws, embeds) = TestEditor::with_test_embeds(lb(), &md);
    embeds.complete(IMAGE, egui::vec2(40.0, 40.0));
    if prop == Prop::OutlineFresh {
        ws.editor.toggle_outline();
    }
    let mut s = Session {
        ws,
        prop,
        features: *features,
        events,
        docs: ByteSource::new(docs),
        save: None,
        latest: (prop == Prop::LatestVersion).then(|| Latest::new(&md)),
        disk: md,
        saves: 0,
        remotes: 0,
        remote_seq: 0,
        left_tier: false,
    };
    s.frame(false, log)?;
    if events >= Events::Scrolled {
        let wheel = 200.0 * (1 + src.draw(12)) as f32;
        log(format!("scroll down {wheel}pt"));
        scroll::scroll(&mut s.ws, wheel);
        s.frame(true, log)?;
    }
    if events >= Events::Held {
        if let Some(at) = pick_visible(&s.ws, &mut src) {
            let caret = Grapheme(at.source_range.start().0 + src.draw(at.len().max(1)));
            log(format!("caret at {caret:?}"));
            s.ws.push(Event::Select {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(caret),
                    end: Location::Grapheme(caret),
                },
            });
            s.frame(true, log)?;
        }
    }
    for _ in 0..3 * event_count(&mut src) {
        s.step(&mut src, log)?;
        if s.left_tier {
            return Ok(());
        }
    }
    if prop == Prop::OutlineFresh {
        outline_fresh(&mut s.ws, log)?;
    }
    Ok(())
}

impl Session<'_> {
    fn seq(&self) -> usize {
        self.ws.editor.edit.renderer.buffer.current.seq
    }

    /// A frame, then frames until the view settles. Only `input` may change
    /// the text.
    fn frame(&mut self, input: bool, log: &mut dyn FnMut(String)) -> Result<(), String> {
        self.ws.enter_frame();
        settle(&mut self.ws);
        self.observe(input, log)
    }

    /// One frame of pointer input.
    fn pointer(&mut self, input: Vec<Egui>, log: &mut dyn FnMut(String)) -> Result<(), String> {
        self.ws.enter_frame_with_input(input);
        self.observe(true, log)
    }

    fn observe(&mut self, input: bool, log: &mut dyn FnMut(String)) -> Result<(), String> {
        match &mut self.latest {
            Some(latest) => latest.framed(self.ws.get_text(), input, log),
            None => Ok(()),
        }
    }

    fn step(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
        match src.bias(&[6, 3, 2, 2, 2, 2]) {
            0 => self.local(src, log),
            1 => self.remote(src, log),
            2 if self.events >= Events::Saves => self.save(log),
            3 if self.undoes() => {
                let before = self.ws.get_text().to_string();
                let undone = self.ws.editor.edit.renderer.buffer.can_undo();
                self.undo(log)?;
                if self.prop == Prop::UndoRedo && undone && !self.left_tier {
                    log("redo".into());
                    self.ws.push(Event::Redo);
                    self.frame(true, log)?;
                    if self.ws.get_text() != before {
                        let text = self.ws.get_text();
                        return Err(format!("undo, redo gives {text:?}, not {before:?}"));
                    }
                }
                Ok(())
            }
            4 if self.events >= Events::Held => self.drag(src, log),
            _ => {
                log("frame".into());
                self.frame(false, log)
            }
        }
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
        self.ws.push(Event::Undo);
        self.frame(true, log)?;
        let across = self.remotes > 0 && self.seq() < self.remote_seq;
        if self.events < Events::Undo && across {
            log("the undo reached back across a remote version".into());
            self.left_tier = true;
        }
        Ok(())
    }

    /// One generated input event, then a frame.
    fn local(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
        let event = event_with(src, &self.ws, &self.features);
        if matches!(event, Event::Undo) {
            return if self.undoes() { self.undo(log) } else { self.frame(false, log) };
        }
        log(format!("local {event:?}"));
        self.ws.push(event);
        self.frame(true, log)
    }

    /// Other devices' versions of the note: one or more, each landing with
    /// a frame, all in one frame, or collapsed to the last.
    fn remote(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
        if self.events < Events::Reading || self.save.is_some() {
            return Ok(());
        }
        if self.events < Events::Typing {
            self.save(log)?;
            self.save(log)?;
        }
        let versions = 1 + src.bias(&[4, 2, 1]);
        let delivered = src.bias(&[3, 1, 1]);
        let mut view = self.view();
        for v in 0..versions {
            let version = self.version(src, log);
            if delivered != 2 || v + 1 == versions {
                self.land(version, log)?;
            } else {
                self.disk = version;
            }
            if delivered == 0 {
                self.frame(false, log)?;
                self.judge(view, log)?;
                view = self.view();
            }
        }
        if delivered != 0 {
            log(format!(
                "{} versions land in one frame",
                if delivered == 1 { versions } else { 1 }
            ));
            self.frame(false, log)?;
            self.judge(view, log)?;
        }
        Ok(())
    }

    /// The view before versions land, for the properties that judge it.
    fn view(&mut self) -> Option<scroll::Before> {
        matches!(self.prop, Prop::TextStays | Prop::CaretStays)
            .then(|| scroll::Before::new(&mut self.ws, self.prop))
            .flatten()
    }

    fn judge(
        &mut self, view: Option<scroll::Before>, log: &mut dyn FnMut(String),
    ) -> Result<(), String> {
        let scrolled = self.events >= Events::Scrolled;
        view.map_or(Ok(()), |view| view.judge(&mut self.ws, self.prop, scrolled, log))
    }

    /// Lands another device's `version` at the top of the next frame, as
    /// the workspace applies a finished load.
    fn land(&mut self, version: String, log: &mut dyn FnMut(String)) -> Result<(), String> {
        // a reload first applies input still queued, as a drop's move
        let ctx = self.ws.editor.edit.renderer.ctx.clone();
        self.ws.editor.edit.apply_queued(&ctx);
        self.observe(true, log)?;
        self.remotes += 1;
        self.disk = version.clone();
        self.ws
            .editor
            .reload(version, Some([self.remotes as u8; 32]));
        self.remote_seq = self.seq();
        match &mut self.latest {
            Some(latest) => latest.landed(&self.disk, self.ws.get_text(), log),
            None => Ok(()),
        }
    }

    /// Another device's next version: an edit at a spot of the note as
    /// shown, carried onto the version on disk without the note's unsaved
    /// edits, or the version on disk reworked whole.
    fn version(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> String {
        let text = self.ws.get_text().to_string();
        let bounds: Vec<usize> = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([text.len()])
            .collect();
        let last = bounds.len() - 1;
        // from Scrolled the spot may be wholly above or below the view
        let view = if self.events >= Events::Scrolled { view_range(&self.ws) } else { None };
        let (lo, hi) = match (view, src.bias(&[2, 1, 1])) {
            (Some((lo, _)), 1) => (0, lo.0.min(last)),
            (Some((_, hi)), 2) => (hi.0.min(last), last),
            _ => (0, last),
        };
        let a = lo + src.draw(hi - lo + 1);
        let b = (a + src.draw(40)).min(hi);
        let disk = self.disk.clone();
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
                gen_doc(&mut self.docs, &self.features)
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
    /// landing without a frame after it. `None` while a save is in flight or
    /// edits are unsaved, since a merge may drop an insert into one, or if no
    /// spot keeps it whole.
    fn remote_marker(
        &mut self, src: &mut ByteSource, log: &mut dyn FnMut(String),
    ) -> Result<Option<String>, String> {
        if self.save.is_some() || self.ws.get_text() != self.disk {
            return Ok(None);
        }
        let marker = format!("⟦{}⟧", self.remotes + 1);
        let disk = &self.disk;
        let images = image_ranges(disk);
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
            return Ok(None);
        }
        let at = bounds[src.draw(bounds.len())];
        log(format!("remote inserts {marker:?} at byte {at} as the drop lets go"));
        let mut next = disk.clone();
        next.insert_str(at, &marker);
        self.land(next, log)?;
        Ok(Some(marker))
    }

    /// Launches a save, or records the one in flight, as the task manager
    /// does at the top of a frame.
    fn save(&mut self, log: &mut dyn FnMut(String)) -> Result<(), String> {
        match self.save.take() {
            None => {
                let (_, text) = self.ws.editor.flush_for_save();
                let seq = self.seq();
                log(format!("save launches at seq {seq}"));
                if let Some(latest) = &mut self.latest {
                    latest.launched();
                }
                self.save = Some((seq, text));
            }
            Some((seq, text)) => {
                self.saves += 1;
                log(format!("save of seq {seq} lands"));
                if let Some(latest) = &mut self.latest {
                    latest.saved(&text);
                }
                self.disk = text.clone();
                self.ws.editor.saved([200 + self.saves; 32], text);
            }
        }
        self.frame(false, log)
    }

    /// Lifts an image with the mouse, holds it through local edits, saves and
    /// remote versions, and drops it somewhere.
    fn drag(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
        let Some(from) = image_center(&self.ws, src) else { return Ok(()) };
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
        let drag = self.ws.editor.edit.in_progress_image_drag.as_ref();
        let Some(grab) = drag.map(|d| d.grab_offset) else {
            return self.pointer(vec![button(from, false)], log);
        };
        let lift = egui::vec2(0.0, grab.y + self.ws.editor.edit.renderer.drop_lift());
        log("drag holds".into());
        for _ in 0..src.draw(4) {
            match src.bias(&[1, 1, 1]) {
                0 => self.remote(src, log)?,
                1 => self.save(log)?,
                _ => self.local(src, log)?,
            }
        }
        let segs = &self.ws.editor.edit.renderer.buffer.current.segs;
        let at = Grapheme(src.draw(segs.last_cursor_position().0 + 1));
        let to = self
            .ws
            .editor
            .edit
            .cursor_line(at)
            .map(|[top, bot]| Pos2::new(top.x - 1.0, (top.y + bot.y) / 2.0))
            .unwrap_or(from);
        log(format!("drag drops at offset {at:?}"));
        self.pointer(vec![Egui::PointerMoved(to)], log)?;
        self.pointer(vec![Egui::PointerMoved(to + lift)], log)?;
        let before = self.ws.get_text().to_string();
        self.pointer(vec![button(to + lift, false)], log)?;
        let dropped = self.ws.editor.edit.in_progress_image_drag.as_ref();
        let dropped = dropped
            .filter(|d| d.dropped)
            .and_then(|d| Some((d.image, d.target?)));
        // a version can land before the next frame moves the image
        let marker = if src.bias(&[1, 1]) == 1 { self.remote_marker(src, log)? } else { None };
        self.frame(true, log)?;
        let mut after = self.ws.get_text().to_string();
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
fn image_center(ws: &TestEditor, src: &mut ByteSource) -> Option<Pos2> {
    let images: Vec<Pos2> = ws
        .editor
        .edit
        .renderer
        .fragments
        .iter()
        .filter(|f| matches!(f.content, FragmentContent::Embed { .. }))
        .map(|f| f.rect.center())
        .filter(|c| (0.0..VIEW.y).contains(&c.y))
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
fn outline_fresh(ws: &mut TestEditor, log: &mut dyn FnMut(String)) -> Result<(), String> {
    ws.enter_frame();
    let rows = |ws: &TestEditor| {
        ws.editor
            .outline_rows
            .as_ref()
            .map(|(_, rows)| {
                rows.iter()
                    .map(|r| (r.level, r.text.clone(), r.start, r.span))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let text = ws.get_text().to_string();
    let mut fresh = TestEditor::with_lb(lb(), &text);
    fresh.editor.toggle_outline();
    fresh.enter_frame();
    let (kept, opened) = (rows(ws), rows(&fresh));
    log(format!("outline kept {kept:?}, fresh {opened:?}"));
    if kept != opened {
        return Err(format!("outline kept {kept:?}, a fresh open lists {opened:?} for {text:?}"));
    }
    Ok(())
}
