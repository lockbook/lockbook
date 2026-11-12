//! Property tests over an open note while background results land between
//! frames: saves completing and other devices' or programs' versions of the
//! note. Each property runs at a [`Features`] document tier and an
//! [`Events`] tier, the highest where it holds.

mod interleave;
mod reload;
mod scroll;

use std::sync::OnceLock;

use lb_rs::blocking::Lb;

use super::doc_gen::Features;
use super::harness::in_memory_core;

/// What happens to the open note besides its document. Each tier adds to
/// the one below, so a property that holds at a tier holds below it. Only
/// the interleave driver saves; the others leave the note unedited until
/// [`Events::Typing`], as a save would.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Events {
    /// Typing, deletes, selections and undo.
    #[expect(dead_code, reason = "the floor: no row sits this low")]
    Local,
    /// Saves launch and land.
    Saves,
    /// Remote versions land, each on a just-saved note.
    Reading,
    /// Remote versions land over unsaved edits.
    Typing,
    /// Undo and redo reach back across remote versions.
    Undo,
    /// A caret placed on screen is held while remote versions land.
    Held,
    /// The view is scrolled down a tall note and remote versions land above it.
    Scrolled,
}

const IMAGE: &str = "https://x.test/a.png";

/// One core shared by every seed.
fn lb() -> Lb {
    static LB: OnceLock<Lb> = OnceLock::new();
    LB.get_or_init(in_memory_core).clone()
}

// property → document tier → event tier → seeds

#[test]
fn undo_then_redo_restores_the_text() {
    interleave::run(interleave::Prop::UndoRedo, Features::all(), Events::Undo, 1000);
}

#[test]
fn the_outline_matches_a_fresh_open() {
    interleave::run(interleave::Prop::OutlineFresh, Features::all(), Events::Undo, 600);
}

#[test]
fn the_note_shows_the_latest_version() {
    reload::run(Features::all(), Events::Typing, 300);
}

#[test]
fn text_on_screen_stays_put() {
    scroll::run(scroll::Prop::TextStays, Features::all(), Events::Held, 300);
}

#[test]
fn the_caret_stays_on_screen() {
    scroll::run(scroll::Prop::CaretStays, Features::all(), Events::Held, 300);
}

/// Runs `check` under the shared seed loop; a failure reproduces with the
/// check's own log.
fn run<C>(seeds: u64, buf_len: usize, check: C)
where
    C: Fn(&[u8], &mut dyn FnMut(String)) -> Result<(), String>,
{
    crate::test_utils::prop::run(
        seeds,
        buf_len,
        |b| caught(|| check(b, &mut |_| {})),
        |b| {
            let mut log = vec![];
            let result = caught(|| check(b, &mut |line| log.push(line)));
            format!("{}\n=> {result:?}", log.join("\n"))
        },
    );
}

/// A panic as a failure, so the seed loop shrinks it.
fn caught(check: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(check)).unwrap_or_else(|panic| {
        let message = panic.downcast_ref::<String>().map(String::as_str);
        let message = message.or(panic.downcast_ref::<&str>().copied());
        Err(format!("panicked: {}", message.unwrap_or("with no message")))
    })
}
