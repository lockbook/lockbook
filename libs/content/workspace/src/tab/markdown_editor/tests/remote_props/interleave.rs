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

use egui::{Event as Egui, Modifiers, PointerButton, Pos2};
use lb_rs::model::text::offset_types::Grapheme;
use unicode_segmentation::UnicodeSegmentation as _;

use super::super::super::input::Event;
use super::super::super::widget::utils::wrap_layout::FragmentContent;
use super::super::doc_gen::{Features, gen_doc};
use super::super::edit_props::{event_count, event_with};
use super::super::harness::TestEditor;
use super::{Events, IMAGE, lb};
use crate::test_utils::byte_source::ByteSource;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Prop {
    /// Undo then redo puts the text back as it was.
    UndoRedo,
    /// The outline lists what a fresh open of the same text lists.
    OutlineFresh,
    /// A drop moves only the image: same characters, same images.
    DropMovesImage,
}

/// From [`Events::Held`] the note has images, and one is lifted and held
/// through local and remote edits, then dropped.
pub(super) fn run(prop: Prop, features: Features, events: Events, seeds: u64) {
    assert!(events <= Events::Held, "nothing here scrolls");
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
    let md = if events >= Events::Held {
        doc_with_images(&mut src, features)
    } else {
        gen_doc(&mut src, features)
    };
    log(format!("doc {md:?}"));
    let (mut ws, embeds) = TestEditor::with_test_embeds(lb(), &md);
    embeds.complete(IMAGE, egui::vec2(40.0, 40.0));
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
        4 if s.events >= Events::Held => drag(s, src, prop, log),
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

/// Lifts an image with the mouse, holds it through local edits, saves and
/// remote versions, and drops it somewhere.
fn drag(
    s: &mut Session, src: &mut ByteSource, prop: Prop, log: &mut dyn FnMut(String),
) -> Result<(), String> {
    let Some(from) = image_center(&s.ws, src) else { return Ok(()) };
    let button = |pos, pressed| Egui::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    log(format!("drag lifts the image at {from:?}"));
    s.ws.enter_frame_with_input(vec![Egui::PointerMoved(from)]);
    s.ws.enter_frame_with_input(vec![button(from, true)]);
    s.ws.enter_frame_with_input(vec![Egui::PointerMoved(from + egui::vec2(20.0, 20.0))]);
    let Some(grab) =
        s.ws.editor
            .edit
            .in_progress_image_drag
            .as_ref()
            .map(|d| d.grab_offset)
    else {
        s.ws.enter_frame_with_input(vec![button(from, false)]);
        return Ok(());
    };
    let lift = egui::vec2(0.0, grab.y + s.ws.editor.edit.renderer.drop_lift());
    log("drag holds".into());
    for _ in 0..src.draw(4) {
        match src.bias(&[1, 1, 1]) {
            0 => remote(s, src, log)?,
            1 => save(s, log)?,
            _ => local(s, src, log)?,
        }
    }
    let len =
        s.ws.editor
            .edit
            .renderer
            .buffer
            .current
            .segs
            .last_cursor_position()
            .0;
    let at = Grapheme(src.draw(len + 1));
    let to =
        s.ws.editor
            .edit
            .cursor_line(at)
            .map(|[top, bot]| Pos2::new(top.x - 1.0, (top.y + bot.y) / 2.0))
            .unwrap_or(from);
    log(format!("drag drops at offset {at:?}"));
    s.ws.enter_frame_with_input(vec![Egui::PointerMoved(to)]);
    s.ws.enter_frame_with_input(vec![Egui::PointerMoved(to + lift)]);
    let before = s.ws.get_text().to_string();
    s.ws.enter_frame_with_input(vec![button(to + lift, false)]);
    s.ws.enter_frame();
    let after = s.ws.get_text();
    if after != before {
        log("the drop moved the image".into());
    }
    if prop == Prop::DropMovesImage {
        let token = format!("![a]({IMAGE})");
        let sorted = |t: &str| {
            let mut c: Vec<char> = t.chars().collect();
            c.sort_unstable();
            c
        };
        if sorted(&before) != sorted(after)
            || before.matches(&token).count() != after.matches(&token).count()
        {
            return Err(format!("the drop turned {before:?} into {after:?}"));
        }
    }
    Ok(())
}

/// The center of one of the drawn images, if any draws.
fn image_center(ws: &TestEditor, src: &mut ByteSource) -> Option<Pos2> {
    let images: Vec<Pos2> = ws
        .editor
        .edit
        .renderer
        .fragments
        .iter()
        .filter(|f| matches!(f.content, FragmentContent::Embed { .. }))
        .map(|f| f.rect.center())
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
