//! What a person types is never lost. Typing drops unique markers `⟪n⟫` in
//! the note and another device drops `⟦n⟧` in the version on disk, while
//! saves and loads finish in any order the task manager lets through and
//! the tab closes and reopens. A marker leaves only by the person's own
//! input; once saves settle the note on disk has every other marker once,
//! and a fresh workspace opens it in the editor.

use std::collections::BTreeSet;

use lb_rs::model::text::offset_types::Grapheme;
use regex::Regex;
use unicode_segmentation::UnicodeSegmentation as _;

use super::super::super::input::{Event, Location, Region};
use super::super::doc_gen::{Features, gen_doc};
use super::super::edit_props::{event_count, event_with};
use super::note::Note;
use super::{Events, Host};
use crate::test_utils::byte_source::ByteSource;

pub(super) fn run(features: Features, events: Events, seeds: u64) {
    assert!(events <= Events::Undo, "nothing here is held or scrolled");
    super::run(seeds, 1024, |b, log| check(b, &features, events, log));
}

struct Session {
    note: Note,
    features: Features,
    events: Events,
    locals: usize,
    remotes: usize,
    /// The markers the note must keep: every one written, less those the
    /// person's input removed.
    kept: BTreeSet<String>,
}

fn markers(text: &str) -> Vec<String> {
    let marker = Regex::new(r"⟪\d+⟫|⟦\d+⟧").unwrap();
    marker
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect()
}

impl Session {
    /// The markers the editor shows, if the note is open.
    fn shown(&mut self) -> Option<Vec<String>> {
        let md = self.note.shown()?;
        Some(markers(&md.edit.renderer.buffer.current.text))
    }

    /// A frame with no input: every kept marker the editor has seen is
    /// still there, once.
    fn frame(&mut self, log: &mut dyn FnMut(String)) -> Result<(), String> {
        log("frame".into());
        self.note.frame_after(0.0, vec![]);
        self.check_shown()
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
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, features).replace('\0', "");
    log(format!("doc {md:?}"));
    let note = Note::open(&md);
    let mut s =
        Session { note, features: *features, events, locals: 0, remotes: 0, kept: BTreeSet::new() };
    for _ in 0..8 * event_count(&mut src) {
        step(&mut s, &mut src, log)?;
    }

    // saves settle: the note reopens, and every tab's edits are saved,
    // closed ones the workspace still holds included
    if s.note.shown().is_none() {
        s.note.settle(log);
        if s.note.shown().is_none() {
            s.note.reopen(log);
        }
    }
    for _ in 0..10 {
        s.note.save(log);
        s.frame(log)?;
        s.note.settle(log);
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
            s.kept.insert(marker);
            if reading {
                s.frame(log)?;
                s.note.settle(log);
                s.check_shown()?;
            }
            Ok(())
        }
        3 if s.events >= Events::Saves => {
            s.note.save(log);
            Ok(())
        }
        4 if s.events >= Events::Saves => {
            s.note.run(src, log);
            Ok(())
        }
        5 if s.events >= Events::Saves => {
            if s.note.ran() > 0 {
                let i = src.draw(s.note.ran());
                s.note.record_at(i, log);
            }
            Ok(())
        }
        6 if open && s.events >= Events::Saves => {
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
