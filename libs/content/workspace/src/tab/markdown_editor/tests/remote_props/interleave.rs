//! Generated input with sync results landing between frames. A frame is
//! atomic, so the editor's only concurrency is which of them reach it
//! between frames: a newer version of the note from a sync
//! ([`Editor::reload`]) or a save's completion
//! ([`Editor::saved`]), around frames of local input. The task
//! manager runs one load or save of a file at a time and applies a finished
//! save before the next load starts, so a remote version only lands while
//! no save is in flight.
//!
//! [`Editor::reload`]: super::super::Editor::reload
//! [`Editor::saved`]: super::super::Editor::saved

use unicode_segmentation::UnicodeSegmentation as _;

use super::super::super::input::Event;
use super::super::doc_gen::{Features, gen_doc};
use super::super::edit_props::{event_count, event_with};
use super::super::harness::TestEditor;
use super::{Events, lb};
use crate::test_utils::byte_source::ByteSource;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Prop {
    /// Undo then redo puts the text back as it was.
    UndoRedo,
    /// The outline lists what a fresh open of the same text lists.
    OutlineFresh,
}

pub(super) fn run(prop: Prop, features: Features, events: Events, seeds: u64) {
    assert!(events <= Events::Undo, "nothing here is held or scrolls");
    super::run(seeds, 256, |b, log| check(b, prop, &features, events, log));
}

/// The note as both sides last agreed on it, and the save in flight.
struct Session {
    ws: TestEditor,
    features: Features,
    events: Events,
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
}

impl Session {
    fn seq(&self) -> usize {
        self.ws.editor.edit.renderer.buffer.current.seq
    }

    /// Below [`Events::Undo`], only local edits since the last remote
    /// version are undone.
    fn undoes(&self) -> bool {
        self.events >= Events::Undo || self.remotes == 0 || self.seq() > self.remote_seq
    }

    /// An undo, then a frame. Undo groups typing, so one can still reach
    /// across a remote version; the run then stops, as it left its tier.
    fn undo(&mut self, log: &mut dyn FnMut(String)) {
        log("undo".into());
        self.ws.push(Event::Undo);
        self.ws.enter_frame();
        let across = self.remotes > 0 && self.seq() < self.remote_seq;
        if self.events < Events::Undo && across {
            log("the undo reached back across a remote version".into());
            self.left_tier = true;
        }
    }
}

fn check(
    buf: &[u8], prop: Prop, features: &Features, events: Events, log: &mut dyn FnMut(String),
) -> Result<(), String> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, features);
    log(format!("doc {md:?}"));
    let mut ws = TestEditor::with_lb(lb(), &md);
    if prop == Prop::OutlineFresh {
        ws.editor.toggle_outline();
    }
    ws.enter_frame();
    let mut s = Session {
        ws,
        features: *features,
        events,
        save: None,
        disk: md,
        saves: 0,
        remotes: 0,
        remote_seq: 0,
        left_tier: false,
    };
    let steps = 3 * event_count(&mut src);
    for _ in 0..steps {
        step(&mut s, &mut src, prop, log)?;
        if s.left_tier {
            return Ok(());
        }
    }
    if prop == Prop::OutlineFresh {
        outline_fresh(&mut s.ws, log)?;
    }
    Ok(())
}

fn step(
    s: &mut Session, src: &mut ByteSource, prop: Prop, log: &mut dyn FnMut(String),
) -> Result<(), String> {
    match src.bias(&[6, 3, 2, 2, 2, 2]) {
        0 => local(s, src, log),
        1 => remote(s, src, log),
        2 if s.events >= Events::Saves => save(s, log),
        3 if s.undoes() => {
            let before = s.ws.get_text().to_string();
            let undone = s.ws.editor.edit.renderer.buffer.can_undo();
            s.undo(log);
            if prop == Prop::UndoRedo && undone && !s.left_tier {
                log("redo".into());
                s.ws.push(Event::Redo);
                s.ws.enter_frame();
                if s.ws.get_text() != before {
                    return Err(format!("undo, redo gives {:?}, not {before:?}", s.ws.get_text()));
                }
            }
            Ok(())
        }
        _ => {
            log("frame".into());
            s.ws.enter_frame();
            Ok(())
        }
    }
}

/// One generated input event, then a frame.
fn local(s: &mut Session, src: &mut ByteSource, log: &mut dyn FnMut(String)) -> Result<(), String> {
    let buf = &s.ws.editor.edit.renderer.buffer.current;
    let (cursor, len) = (buf.selection.1, buf.segs.last_cursor_position().0);
    let event = event_with(src, cursor, len, &s.features);
    if matches!(event, Event::Undo) {
        if s.undoes() {
            s.undo(log);
        } else {
            s.ws.enter_frame();
        }
        return Ok(());
    }
    log(format!("local {event:?}"));
    s.ws.push(event);
    s.ws.enter_frame();
    Ok(())
}

/// Another device's edit to the version on disk, applied at the top of the
/// next frame as the workspace applies a finished load.
fn remote(
    s: &mut Session, src: &mut ByteSource, log: &mut dyn FnMut(String),
) -> Result<(), String> {
    if s.events < Events::Reading || s.save.is_some() {
        return Ok(());
    }
    if s.events < Events::Typing {
        save(s, log)?;
        save(s, log)?;
    }
    let disk = s.disk.clone();
    let bounds: Vec<usize> = disk
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain([disk.len()])
        .collect();
    let a = bounds[src.draw(bounds.len())];
    let b = bounds[(bounds.partition_point(|&i| i < a) + src.draw(8)).min(bounds.len() - 1)];
    s.remotes += 1;
    let marker = format!("⟦{}⟧", s.remotes);
    let (range, text) = match src.bias(&[4, 1, 1]) {
        0 => (a..a, marker),
        1 => (a..b, String::new()),
        _ => (a..b, marker),
    };
    let mut next = disk;
    log(format!(
        "remote replaces {:?} at byte {} with {text:?}",
        &next[range.clone()],
        range.start
    ));
    next.replace_range(range, &text);
    let hmac = [s.remotes as u8; 32];
    s.disk = next.clone();
    s.ws.editor.reload(next, Some(hmac));
    s.ws.enter_frame();
    s.remote_seq = s.seq();
    Ok(())
}

/// Launches a save, or records the one in flight, as the task manager does
/// at the top of a frame.
fn save(s: &mut Session, log: &mut dyn FnMut(String)) -> Result<(), String> {
    match s.save.take() {
        None => {
            let (_, text) = s.ws.editor.flush_for_save();
            let seq = s.ws.editor.edit.renderer.buffer.current.seq;
            log(format!("save launches at seq {seq}"));
            s.save = Some((seq, text));
        }
        Some((seq, text)) => {
            s.saves += 1;
            log(format!("save of seq {seq} lands"));
            s.disk = text.clone();
            s.ws.editor.saved([200 + s.saves; 32], text);
        }
    }
    s.ws.enter_frame();
    Ok(())
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
