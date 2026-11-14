//! What a person types is never lost. Typing drops unique markers `⟪n⟫` in
//! the note and another device drops `⟦n⟧` in the version on disk, while
//! saves and loads finish in any order the task manager lets through and
//! the tab closes and reopens. From Held an image may be held across all of
//! that and dropped, which moves its text alone; from Scrolled the note is
//! tall and scrolled down. A marker leaves only by the person's own
//! input, or where a landing version loses to the person's edit at one
//! spot, as lb-rs's three-way merge drops it or by touching what the person
//! changed since the shared version: two edits at one spot keep a winner.
//! Once saves settle the note on disk
//! has every other marker once, and a fresh workspace opens it in the editor.

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;

use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::Grapheme;
use regex::Regex;
use similar::{DiffOp, TextDiff};
use unicode_segmentation::UnicodeSegmentation as _;

use egui::{Event as Egui, Modifiers, PointerButton, Pos2};

use super::super::super::input::{Event, Location, Region};
use super::super::super::widget::utils::wrap_layout::FragmentContent;
use super::super::doc_gen::{Features, gen_doc};
use super::super::edit_props::{event_count, event_with};
use super::note::{Landed, Note};
use super::scroll::{self, BLOCK, BLOCKS, long_doc, pick_visible};
use super::{Events, Host, IMAGE};
use crate::test_utils::byte_source::ByteSource;

/// The run's choices; from Held the opening note is made from the bytes after.
const CHOICES: usize = 1024;

pub(super) fn run(features: Features, events: Events, seeds: u64) {
    let len = if events >= Events::Held { CHOICES + BLOCK * BLOCKS } else { CHOICES };
    super::run(seeds, len, |b, log| check(b, &features, events, log));
}

struct Session {
    note: Note,
    features: Features,
    events: Events,
    locals: usize,
    remotes: usize,
    /// The markers the note must keep: every one written, less those the
    /// person's input removed or a merge dropped.
    kept: BTreeSet<String>,
    /// Every version written to disk, by its hmac.
    versions: HashMap<DocumentHmac, String>,
    /// An image is lifted and held.
    holding: bool,
}

fn markers(text: &str) -> Vec<String> {
    let marker = Regex::new(r"⟪\d+⟫|⟦\d+⟧").unwrap();
    marker
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect()
}

impl Session {
    /// Notes the version on disk, which every write goes through.
    fn read_disk(&mut self) {
        let (hmac, bytes) = self
            .note
            .t
            .ws
            .core
            .read_document_with_hmac(self.note.id, false)
            .unwrap();
        if let Some(hmac) = hmac {
            self.versions
                .insert(hmac, String::from_utf8(bytes).unwrap());
        }
    }

    /// Each of the note's tabs with its version and text.
    fn tabs(&self) -> Vec<(impl PartialEq, Option<DocumentHmac>, String)> {
        self.note
            .t
            .ws
            .tabs
            .iter()
            .filter(|(_, tab)| tab.id() == Some(self.note.id))
            .filter_map(|(&sid, tab)| {
                let md = tab.markdown()?;
                Some((sid, md.hmac, md.edit.renderer.buffer.current.text.clone()))
            })
            .collect()
    }

    /// A frame with `input`. A tab that loads a version merges it into
    /// its edits since the version it held; a marker lb-rs's three-way merge
    /// of those texts drops, the tab's edits winning overlaps, lost to the
    /// other side's edit at that spot and leaves `kept`.
    fn land(&mut self, input: Vec<Egui>, log: &mut dyn FnMut(String)) {
        let before = self.tabs();
        self.note.frame_after(0.0, input);
        let loads: Vec<String> = self
            .note
            .take_applied()
            .into_iter()
            .filter_map(|l| if let Landed::Load(text) = l { Some(text) } else { None })
            .collect();
        if loads.is_empty() {
            return;
        }
        for (sid, now, shown) in self.tabs() {
            let Some((_, was, local)) = before.iter().find(|(b, ..)| *b == sid) else {
                continue;
            };
            if *was == now {
                continue;
            }
            let base = was.and_then(|h| self.versions.get(&h));
            let remote = now.and_then(|h| self.versions.get(&h));
            let (Some(base), Some(remote)) = (base, remote.filter(|r| loads.contains(r))) else {
                continue;
            };
            let merged = markers(&Buffer::from(base.as_str()).merge(local.clone(), remote.clone()));
            let changed = changed(base, local);
            let shown = markers(&shown);
            for m in markers(local).into_iter().chain(markers(remote)) {
                let touches = || {
                    inserted_at(base, remote, &m)
                        .is_some_and(|(a, b)| changed.iter().any(|&(s, e)| a <= e && b >= s))
                };
                let lost = !merged.contains(&m) || touches();
                if lost && !shown.contains(&m) && self.kept.remove(&m) {
                    log(format!("{m} loses an overlap"));
                }
            }
        }
    }

    /// Runs one launched read or write; a write is a new version.
    fn run(&mut self, src: &mut ByteSource, log: &mut dyn FnMut(String)) {
        self.note.run(src, log);
        self.read_disk();
    }

    /// Runs and records everything in flight, in launch order, framing
    /// until nothing is left.
    fn settle(&mut self, log: &mut dyn FnMut(String)) {
        let mut src = ByteSource::new(&[]);
        for _ in 0..100 {
            if self.note.idle() {
                return;
            }
            while self.note.held() > 0 {
                self.run(&mut src, log);
            }
            while self.note.ran() > 0 {
                self.note.record_at(0, log);
            }
            self.land(vec![], log);
        }
        panic!("the note's loads and saves never settled");
    }

    /// The markers the editor shows, if the note is open.
    fn shown(&mut self) -> Option<Vec<String>> {
        let md = self.note.shown()?;
        Some(markers(&md.edit.renderer.buffer.current.text))
    }

    /// A frame with no input: every kept marker the editor has seen is
    /// still there, once.
    fn frame(&mut self, log: &mut dyn FnMut(String)) -> Result<(), String> {
        log("frame".into());
        self.land(vec![], log);
        self.check_shown()
    }

    /// One frame of pointer input, which removes no marker. A recorded load
    /// applies in a frame of its own first.
    fn pointer(&mut self, input: Vec<Egui>, log: &mut dyn FnMut(String)) -> Result<(), String> {
        if self.note.load_recorded() {
            self.frame(log)?;
        }
        self.land(input, log);
        self.check_shown()
    }

    /// Lifts an image with the mouse, holds it through any steps, and drops
    /// it at a random offset: the image's own text lands where the drop
    /// line put it.
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
        self.holding = true;
        for _ in 0..src.draw(4) {
            step(self, src, log)?;
        }
        self.holding = false;
        if self.note.shown().is_none() {
            log("the drag ends with its tab".into());
            return Ok(());
        }
        // the drop lands outside every marker, which stays whole
        let text = self.note.text();
        let inside: Vec<_> = Regex::new(r"⟪\d+⟫|⟦\d+⟧")
            .unwrap()
            .find_iter(text)
            .map(|m| m.range())
            .collect();
        let spots: Vec<usize> = (text.grapheme_indices(true).map(|(i, _)| i))
            .chain([text.len()])
            .enumerate()
            .filter(|&(_, i)| !inside.iter().any(|r| r.start < i && i < r.end))
            .map(|(g, _)| g)
            .collect();
        let at = Grapheme(spots[src.draw(spots.len())]);
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
        if self.note.load_recorded() {
            self.frame(log)?;
        }
        let before = self.note.text().to_string();
        self.pointer(vec![button(to + lift, false)], log)?;
        let dropped = self.note.editor().edit.in_progress_image_drag.as_ref();
        let dropped = dropped
            .filter(|d| d.dropped)
            .and_then(|d| Some((d.image, d.target?)));
        // the move applies in the frame after the release
        self.frame(log)?;
        let Some((image, to)) = dropped.filter(|&(image, to)| image.0 > to || to > image.1) else {
            return Ok(());
        };
        let graphemes: Vec<&str> = before.graphemes(true).collect();
        let text = graphemes[image.0.0..image.1.0].concat();
        let len = image.1.0 - image.0.0;
        let at = if to > image.1 { to.0 - len } else { to.0 };
        let landed: String = self
            .note
            .text()
            .graphemes(true)
            .skip(at)
            .take(len)
            .collect();
        if landed != text {
            return Err(format!("{text:?} dropped at {to:?} isn't there: {:?}", self.note.text()));
        }
        log("the drop moved the image".into());
        Ok(())
    }

    /// Every kept marker is on disk or in a tab of the note, open or
    /// closed with its edits unsaved; the editor shows none twice.
    fn check_shown(&mut self) -> Result<(), String> {
        if let Some(shown) = self.shown() {
            for m in &shown {
                if shown.iter().filter(|s| *s == m).count() > 1 {
                    return Err(format!("{m} shows twice: {:?}", self.note.text()));
                }
            }
        }
        let mut held = markers(&self.note.disk());
        for text in self.note.tab_texts() {
            held.extend(markers(&text));
        }
        for m in &self.kept {
            if !held.contains(m) {
                return Err(format!("{m} is lost: on disk {:?}", self.note.disk()));
            }
        }
        Ok(())
    }

    /// The person's input, then a frame. A recorded load applies in a frame
    /// of its own first, so only the input removes markers.
    fn input(&mut self, mut event: Event, log: &mut dyn FnMut(String)) -> Result<(), String> {
        if self.note.load_recorded() {
            self.frame(log)?;
        }
        if let Event::Replace { text, .. } = &mut event {
            // a NUL makes the note binary to the workspace
            text.retain(|c| c != '\0');
        }
        log(format!("local {event:?}"));
        let before = self.shown().unwrap_or_default();
        self.note.t.push(event);
        self.note.frame_after(0.0, vec![]);
        let after = self.shown().unwrap_or_default();
        for m in before.iter().filter(|m| !after.contains(m)) {
            log(format!("{m} deleted"));
            self.kept.remove(m);
        }
        for m in after.iter().filter(|m| !before.contains(m)) {
            self.kept.insert(m.clone());
        }
        Ok(())
    }
}

/// A byte offset in `text` where `marker` can go: outside any marker, and
/// with ends unlike the text beside it, so a diff can't find it shifted.
fn spot(text: &str, marker: &str, src: &mut ByteSource) -> Option<usize> {
    let inside: Vec<_> = Regex::new(r"⟪\d+⟫|⟦\d+⟧|!\[[^\[\]\n]*\]\([^()\s]*\)")
        .unwrap()
        .find_iter(text)
        .map(|m| m.range())
        .collect();
    let (first, last) = (marker.chars().next(), marker.chars().last());
    let spots: Vec<usize> = text
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain([text.len()])
        .filter(|&i| !inside.iter().any(|r| r.start < i && i < r.end))
        .filter(|&i| text[i..].chars().next() != first && text[..i].chars().last() != last)
        .collect();
    (!spots.is_empty()).then(|| spots[src.draw(spots.len())])
}

fn check(
    buf: &[u8], features: &Features, events: Events, log: &mut dyn FnMut(String),
) -> Result<(), String> {
    let (choices, doc) = buf.split_at(buf.len().min(CHOICES));
    let mut src = ByteSource::new(choices);
    let md = if events >= Events::Scrolled {
        long_doc(doc, features)
    } else if events >= Events::Held {
        doc_with_images(&mut ByteSource::new(doc), features)
    } else {
        gen_doc(&mut src, features)
    };
    // a NUL makes the note binary to the workspace
    let md = md.replace('\0', "");
    log(format!("doc {md:?}"));
    let note = Note::open(&md);
    let mut s = Session {
        note,
        features: *features,
        events,
        locals: 0,
        remotes: 0,
        kept: BTreeSet::new(),
        versions: HashMap::new(),
        holding: false,
    };
    s.read_disk();
    if events >= Events::Scrolled {
        let wheel = 200.0 * (1 + src.draw(12)) as f32;
        log(format!("scroll down {wheel}pt"));
        scroll::scroll(&mut s.note, wheel);
        scroll::settle(&mut s.note);
    }
    if events >= Events::Held {
        if let Some(at) = pick_visible(&s.note, &mut src) {
            let caret = Grapheme(at.source_range.0.0 + src.draw(at.len().max(1)));
            let caret = Location::Grapheme(caret);
            let region = Region::BetweenLocations { start: caret, end: caret };
            s.input(Event::Select { region }, log)?;
        }
    }
    for _ in 0..8 * event_count(&mut src) {
        step(&mut s, &mut src, log)?;
    }

    // saves settle: the note reopens, and every tab's edits are saved,
    // closed ones the workspace still holds included
    if s.note.shown().is_none() {
        s.settle(log);
        if s.note.shown().is_none() {
            s.note.reopen(log);
        }
    }
    for _ in 0..10 {
        s.note.save(log);
        s.frame(log)?;
        s.settle(log);
        s.check_shown()?;
        let disk = s.note.disk();
        if s.note.shown().is_some() && s.note.tab_texts().iter().all(|t| *t == disk) {
            break;
        }
    }
    let disk = s.note.disk();
    if let Some(tab) = s.note.tab_texts().into_iter().find(|t| *t != disk) {
        return Err(format!("saves never settle: {disk:?} on disk, a tab holds {tab:?}"));
    }
    let on_disk = markers(&disk);
    for m in &s.kept {
        if on_disk.iter().filter(|d| *d == m).count() != 1 {
            return Err(format!("{m} isn't on disk once: {disk:?}"));
        }
    }
    let fresh = s.note.fresh()?;
    if fresh != disk {
        return Err(format!("a fresh open shows {fresh:?}, not {disk:?}"));
    }
    Ok(())
}

fn step(s: &mut Session, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
    let open = s.note.shown().is_some();
    match src.bias(&[3, 2, 2, 2, 3, 3, 1, 1, 3]) {
        // a marker typed into the note
        0 if open => {
            // the spot is chosen in the text the input lands on
            if s.note.load_recorded() {
                s.frame(log)?;
            }
            s.locals += 1;
            let marker = format!("⟪{}⟫", s.locals);
            let text = s.note.text();
            let Some(at) = spot(text, &marker, src) else { return Ok(()) };
            let at = Grapheme(text[..at].graphemes(true).count());
            let at = Location::Grapheme(at);
            let region = Region::BetweenLocations { start: at, end: at };
            s.input(Event::Replace { region, text: marker, advance_cursor: true }, log)
        }
        // any other input: typing, deletes, selections, formatting, undo
        1 if open && s.events >= Events::Held && !s.holding && src.bias(&[3, 1]) == 1 => {
            s.drag(src, log)
        }
        1 if open => {
            let features = s.features;
            let event = event_with(src, s.note.editor(), &features);
            if matches!(event, Event::Undo | Event::Redo) && s.events < Events::Undo {
                return Ok(());
            }
            s.input(event, log)
        }
        // another device inserts a marker in the version on disk
        2 if s.events >= Events::Reading => {
            s.remotes += 1;
            let marker = format!("⟦{}⟧", s.remotes);
            let mut disk = s.note.disk();
            // below Typing it lands on a saved note before more input
            let reading = s.events < Events::Typing;
            if reading && (!open || s.note.tab_texts().iter().any(|t| *t != disk)) {
                return Ok(());
            }
            let Some(at) = spot(&disk, &marker, src) else { return Ok(()) };
            log(format!("remote inserts {marker} at byte {at}"));
            disk.insert_str(at, &marker);
            s.note.remote(&disk);
            s.read_disk();
            s.kept.insert(marker);
            if reading {
                s.frame(log)?;
                s.settle(log);
                s.check_shown()?;
            }
            Ok(())
        }
        3 if s.events > Events::Local => {
            s.note.save(log);
            Ok(())
        }
        4 if s.events > Events::Local => {
            s.run(src, log);
            Ok(())
        }
        5 if s.events > Events::Local => {
            if s.note.ran() > 0 {
                let i = src.draw(s.note.ran());
                s.note.record_at(i, log);
            }
            Ok(())
        }
        6 if open && s.events > Events::Local => {
            s.note.close(log);
            s.frame(log)
        }
        7 if !open && s.note.t.ws.tab_strip.is_empty() => {
            s.note.reopen(log);
            s.frame(log)
        }
        _ => s.frame(log),
    }
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

/// The stretches of `base`, in chars, its change to `local` covers, each as
/// wide as any alignment of the diff could place it. Where the two are
/// equal there is none.
fn changed(base: &str, local: &str) -> Vec<(usize, usize)> {
    let b: Vec<char> = base.chars().collect();
    let l: Vec<char> = local.chars().collect();
    let mut hunks: Vec<(Range<usize>, Range<usize>)> = vec![];
    for op in TextDiff::from_chars(base, local).ops() {
        if matches!(op, DiffOp::Equal { .. }) {
            continue;
        }
        let (old, new) = (op.old_range(), op.new_range());
        match hunks.last_mut() {
            Some((o, n)) if o.end == old.start && n.end == new.start => {
                (o.end, n.end) = (old.end, new.end);
            }
            _ => hunks.push((old, new)),
        }
    }
    hunks
        .into_iter()
        .map(|(old, new)| widest(&b, old, &l[new]))
        .collect()
}

/// How far the hunk replacing `b[old]` with `ins` reaches: as far as it
/// slides, by one char while the char it passes is where both sides end
/// alike, which leaves the result as it was, then across the run of equal
/// chars on each side.
fn widest(b: &[char], old: Range<usize>, ins: &[char]) -> (usize, usize) {
    let (mut s, mut e, mut i) = (old.start, old.end, ins.to_vec());
    while s > 0 {
        let beside = b[s - 1];
        let del_end = if e > s { b[e - 1] } else { beside };
        if i.last().copied().unwrap_or(beside) != del_end {
            break;
        }
        i.insert(0, beside);
        i.pop();
        (s, e) = (s - 1, e - 1);
    }
    let lo = s;
    let (mut s, mut e, mut i) = (old.start, old.end, ins.to_vec());
    while e < b.len() {
        let beside = b[e];
        let del_start = if e > s { b[s] } else { beside };
        if i.first().copied().unwrap_or(beside) != del_start {
            break;
        }
        i.push(beside);
        i.remove(0);
        (s, e) = (s + 1, e + 1);
    }
    let (mut lo, mut hi) = (lo, e);
    let run = |at: usize| b.get(at).copied();
    let left = lo.checked_sub(1).and_then(run);
    while lo > 0 && run(lo - 1) == left {
        lo -= 1;
    }
    let right = run(hi);
    while hi < b.len() && run(hi) == right {
        hi += 1;
    }
    (lo, hi)
}

/// Where in `base`, in chars, `remote` inserted `marker`, if it did.
fn inserted_at(base: &str, remote: &str, marker: &str) -> Option<(usize, usize)> {
    if base.contains(marker) {
        return None;
    }
    let at = remote[..remote.find(marker)?].chars().count();
    TextDiff::from_chars(base, remote)
        .ops()
        .iter()
        .find(|op| !matches!(op, DiffOp::Equal { .. }) && op.new_range().contains(&at))
        .map(|op| (op.old_range().start, op.old_range().end))
}
