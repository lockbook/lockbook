//! The headings of every markdown note, for the link picker to look up in
//! notes it hasn't opened. The file cache holds them; a thread reads every
//! note once, then each note again when it's written.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::service::events::Event;

use crate::file_cache::{FileCache, FilesExt as _};
use crate::tab::markdown_editor::fragment::{Heading, document_headings};

#[derive(Default)]
pub struct NoteIndex {
    headings: HashMap<Uuid, Arc<Vec<Heading>>>,
}

impl NoteIndex {
    /// The headings of note `id`, once it has been read.
    pub fn headings(&self, id: Uuid) -> Option<Arc<Vec<Heading>>> {
        self.headings.get(&id).cloned()
    }

    pub fn insert(&mut self, id: Uuid, headings: Vec<Heading>) {
        self.headings.insert(id, Arc::new(headings));
    }

    /// Drops the notes `files` no longer has.
    pub fn retain_in(&mut self, files: &FileCache) {
        self.headings.retain(|id, _| files.get_by_id(*id).is_some());
    }

    /// Fills `files`' index on a thread of its own: every note now, a note
    /// again when it's written, and every note when another process changed
    /// them.
    pub fn keep(files: Arc<RwLock<FileCache>>, lb: Lb, ctx: egui::Context) {
        thread::spawn(move || {
            let mut events = lb.subscribe();
            read_all(&files, &lb);
            ctx.request_repaint();
            loop {
                match events.blocking_recv() {
                    Ok(Event::DocumentWritten(id, _)) => {
                        let markdown = files.read().unwrap().get_by_id(id).is_some_and(is_note);
                        if let Some(headings) = read(&lb, id).filter(|_| markdown) {
                            files.write().unwrap().notes.insert(id, headings);
                        }
                    }
                    Ok(Event::IpcChangesApplied)
                    | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        read_all(&files, &lb);
                    }
                    Ok(_) => continue,
                    Err(_) => return,
                }
                ctx.request_repaint();
            }
        });
    }
}

fn is_note(f: &lb_rs::model::file::File) -> bool {
    f.is_document() && f.name.ends_with(".md")
}

fn read(lb: &Lb, id: Uuid) -> Option<Vec<Heading>> {
    let bytes = lb.read_document(id, false).ok()?;
    Some(document_headings(&String::from_utf8_lossy(&bytes)))
}

/// Reads every note in `files`, in parallel.
fn read_all(files: &RwLock<FileCache>, lb: &Lb) {
    let ids: Vec<Uuid> = files
        .read()
        .unwrap()
        .iter_files()
        .filter(|f| is_note(f))
        .map(|f| f.id)
        .collect();
    let queue = Mutex::new(ids);
    let read_notes = Mutex::new(vec![]);
    let workers = thread::available_parallelism().map_or(4, |n| n.get());
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(id) = queue.lock().unwrap().pop() {
                    if let Some(headings) = read(lb, id) {
                        read_notes.lock().unwrap().push((id, headings));
                    }
                }
            });
        }
    });
    let mut files = files.write().unwrap();
    for (id, headings) in read_notes.into_inner().unwrap() {
        files.notes.insert(id, headings);
    }
}
