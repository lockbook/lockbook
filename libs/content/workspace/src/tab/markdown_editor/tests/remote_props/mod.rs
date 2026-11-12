//! Property tests over an open note while background results land between
//! frames: saves completing and other devices' or programs' versions of the
//! note. Every property runs under one driver, at a [`Features`] document
//! tier and an [`Events`] tier, the highest where it holds.

mod interleave;
mod reload;
mod scroll;

use std::sync::OnceLock;

use lb_rs::blocking::Lb;

use super::doc_gen::Features;
use super::harness::in_memory_core;

/// What happens to the open note besides its document. Each tier adds to
/// the one below, so a property that holds at a tier holds below it.
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
    /// An image drag, or a caret placed on screen, is held while remote
    /// versions land.
    Held,
    /// The view is scrolled down a tall note and remote versions land above it.
    Scrolled,
}

/// What a row checks of the run.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Prop {
    /// Undo then redo puts the text back as it was.
    UndoRedo,
    /// The outline lists what a fresh open of the same text lists.
    OutlineFresh,
    /// A drop moves only the image: same characters, same images.
    DropMovesImage,
    /// After every frame the note shows the latest version merged with its edits.
    LatestVersion,
    /// Text on screen stays put under an edit wholly above or below it.
    TextStays,
    /// A caret on screen stays on screen.
    CaretStays,
}

const IMAGE: &str = "https://x.test/a.png";

/// One core shared by every seed.
fn lb() -> Lb {
    static LB: OnceLock<Lb> = OnceLock::new();
    LB.get_or_init(in_memory_core).clone()
}

// property → document tier → event tier → seeds. A row's tier is where a
// local run passes every seed; CI's run stops at its time budget, so it
// can't set one.

#[test]
fn undo_then_redo_restores_the_text() {
    interleave::run(Prop::UndoRedo, Features::all(), Events::Scrolled, 2000);
}

#[test]
fn the_outline_matches_a_fresh_open() {
    interleave::run(Prop::OutlineFresh, Features::all(), Events::Scrolled, 1000);
}

#[test]
fn a_drop_moves_only_the_image() {
    interleave::run(Prop::DropMovesImage, Features::all(), Events::Undo, 2000);
}

#[test]
fn the_note_shows_the_latest_version() {
    interleave::run(Prop::LatestVersion, Features::all(), Events::Held, 600);
}

#[test]
fn text_on_screen_stays_put() {
    interleave::run(Prop::TextStays, Features::all(), Events::Scrolled, 300);
}

#[test]
fn the_caret_stays_on_screen() {
    interleave::run(Prop::CaretStays, Features::all(), Events::Scrolled, 300);
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
