//! Where a spoken conversation meets the host's audio engine. One session
//! speaks at a time, from whichever chat; the engine runs on the host's own
//! threads, so it sends what the microphone hears and how far each reply
//! has played through here, and takes the replies' audio and the session's
//! state from here.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use lb_chat::{Cmd, Handle};

static OFFERED: AtomicBool = AtomicBool::new(false);
static LIVE: Mutex<Live> = Mutex::new(Live {
    session: None,
    start: false,
    stop: false,
    flush: false,
    play: VecDeque::new(),
});

struct Live {
    session: Option<Handle>,
    start: bool,
    stop: bool,
    flush: bool,
    play: VecDeque<(u32, Vec<u8>)>,
}

/// What the host takes: whether to start or stop its engine, whether to
/// drop what is queued to play, and the next of one reply's audio.
#[derive(Debug, Default, PartialEq)]
pub struct Out {
    pub start: bool,
    pub stop: bool,
    pub flush: bool,
    pub play: Option<(u32, Vec<u8>)>,
}

fn live() -> std::sync::MutexGuard<'static, Live> {
    LIVE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The host has an audio engine, so chats offer a call.
pub fn offer() {
    OFFERED.store(true, Ordering::Relaxed);
}

pub fn offered() -> bool {
    OFFERED.load(Ordering::Relaxed)
}

/// A session is open; the host should start listening and playing. A
/// call already on ends, since there is one microphone.
pub fn begin(session: Handle) {
    let mut live = live();
    if let Some(older) = live.session.replace(session) {
        older.send(Cmd::Stop);
    }
    live.play.clear();
    live.start = true;
}

/// The session `ended` is over; nothing happens if another has since taken
/// the floor.
pub fn end(ended: &Handle) {
    let mut live = live();
    if !live.session.as_ref().is_some_and(|s| s.is(ended)) {
        return;
    }
    live.session = None;
    live.stop = true;
    live.play.clear();
}

pub fn play(reply: u32, pcm: Vec<u8>) {
    if !pcm.is_empty() {
        live().play.push_back((reply, pcm));
    }
}

/// The user spoke over the reply: what is queued to play is dropped.
pub fn flush() {
    let mut live = live();
    live.play.clear();
    live.flush = true;
}

/// What the microphone heard, as PCM16 mono at 24 kHz.
pub fn audio(pcm: Vec<u8>) {
    if let Some(session) = &live().session {
        session.send(Cmd::Audio(pcm));
    }
}

pub fn played(reply: u32, ms: u64) {
    if let Some(session) = &live().session {
        session.send(Cmd::Played { reply, ms });
    }
}

pub fn hang_up() {
    if let Some(session) = &live().session {
        session.send(Cmd::Stop);
    }
}

/// Takes the flags and the audio queued for the first reply in line; a
/// later reply's waits for the next take, so one take is one reply's.
pub fn take() -> Out {
    let mut live = live();
    let mut out = Out { start: live.start, stop: live.stop, flush: live.flush, play: None };
    (live.start, live.stop, live.flush) = (false, false, false);
    if let Some(&(reply, _)) = live.play.front() {
        let mut pcm = Vec::new();
        while let Some((_, chunk)) = live.play.front().filter(|(r, _)| *r == reply) {
            pcm.extend_from_slice(chunk);
            live.play.pop_front();
        }
        out.play = Some((reply, pcm));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One take carries one reply's audio, whole, and the flags once.
    #[test]
    fn a_take_is_one_replys_audio_and_the_flags_once() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        flush();
        play(1, vec![1, 2]);
        play(1, vec![3]);
        play(2, vec![4]);
        let first = take();
        assert_eq!(
            first,
            Out { flush: true, play: Some((1, vec![1, 2, 3])), ..Default::default() }
        );
        assert_eq!(take(), Out { play: Some((2, vec![4])), ..Default::default() });
        assert_eq!(take(), Out::default());
    }

    /// Ending a session drops what was queued and says stop, but only the
    /// session that has the floor: a newer call ends the older one, whose
    /// end then changes nothing.
    #[test]
    fn ending_drops_the_queue_and_says_stop_for_the_session_with_the_floor() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (older, mut older_cmds) = handle();
        let (newer, _newer_cmds) = handle();
        begin(older.clone());
        play(1, vec![1]);
        begin(newer.clone());
        assert!(matches!(older_cmds.try_recv(), Ok(Cmd::Stop)));
        assert_eq!(take(), Out { start: true, ..Default::default() });
        end(&older);
        assert_eq!(take(), Out::default());
        play(1, vec![1]);
        end(&newer);
        assert_eq!(take(), Out { stop: true, ..Default::default() });
    }

    fn handle() -> (Handle, tokio::sync::mpsc::UnboundedReceiver<Cmd>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (Handle::from_sender(tx), rx)
    }

    static TEST_LOCK: Mutex<()> = Mutex::new(());
}
