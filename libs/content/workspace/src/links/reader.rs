use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use web_time::{Duration, Instant};

use super::{Link, extract};
use crate::file_cache::UuidMap;

/// How long a note that couldn't be fetched is left before another try.
const RETRY: Duration = Duration::from_secs(60);

/// A note's links as read from its text, or `None` if it couldn't be
/// fetched.
pub struct Read {
    pub note: Uuid,
    /// The note's `last_modified` when the read was asked for.
    pub version: u64,
    pub links: Option<Vec<Link>>,
}

/// Reads notes and finds their links, off the UI thread.
pub struct Reader {
    requests: Sender<(Uuid, u64, u64)>,
    reads: Receiver<(u64, Read)>,
    /// The latest request for each note with a read under way: its number
    /// and the version asked for.
    asked: UuidMap<(u64, u64)>,
    next: u64,
    /// When each note that couldn't be fetched was last tried.
    failed: UuidMap<Instant>,
}

impl Reader {
    pub fn new(core: &Lb, ctx: &egui::Context) -> Self {
        let (requests, queue) = mpsc::channel::<(Uuid, u64, u64)>();
        let (done, reads) = mpsc::channel();
        let queue = Arc::new(Mutex::new(queue));
        // no threads to read on in a browser; nothing is read there
        let workers = thread::available_parallelism().map_or(2, |n| n.get().min(4));
        for _ in 0..if cfg!(target_family = "wasm") { 0 } else { workers } {
            let (core, ctx, queue, done) = (core.clone(), ctx.clone(), queue.clone(), done.clone());
            thread::spawn(move || {
                loop {
                    let next = queue.lock().unwrap().recv();
                    let Ok((note, version, number)) = next else { return };
                    // a note that isn't text, or that the parser chokes on,
                    // has no links to read; the worker lives to read the next
                    let links = core.read_document(note, false).ok().map(|bytes| {
                        let text = String::from_utf8(bytes).unwrap_or_default();
                        std::panic::catch_unwind(|| extract(&text)).unwrap_or_default()
                    });
                    if done.send((number, Read { note, version, links })).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            });
        }
        Self { requests, reads, asked: Default::default(), next: 0, failed: Default::default() }
    }

    /// Reads `note`, whose `last_modified` is `version`.
    pub fn read(&mut self, note: Uuid, version: u64) {
        if cfg!(target_family = "wasm") {
            return;
        }
        self.next += 1;
        self.asked.insert(note, (self.next, version));
        let _ = self.requests.send((note, version, self.next));
    }

    /// Whether a read of `note` at `version` is under way.
    pub fn reading(&self, note: Uuid, version: u64) -> bool {
        self.asked
            .get(&note)
            .is_some_and(|asked| asked.1 == version)
    }

    /// Whether `note` couldn't be fetched a moment ago; asking again so soon
    /// would fail the same way.
    pub fn failed_lately(&self, note: Uuid) -> bool {
        self.failed
            .get(&note)
            .is_some_and(|at| at.elapsed() < RETRY)
    }

    /// Whether every read asked for has come back.
    pub fn idle(&self) -> bool {
        self.asked.is_empty()
    }

    /// Finished reads. One that a later request for its note has overtaken
    /// is left out.
    pub fn finished(&mut self) -> Vec<Read> {
        let mut finished = vec![];
        while let Ok((number, read)) = self.reads.try_recv() {
            if self
                .asked
                .get(&read.note)
                .is_some_and(|asked| asked.0 == number)
            {
                self.asked.remove(&read.note);
                match read.links {
                    Some(_) => self.failed.remove(&read.note),
                    None => self.failed.insert(read.note, Instant::now()),
                };
                finished.push(read);
            }
        }
        finished
    }
}
