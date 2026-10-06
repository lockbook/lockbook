//! Keeps the link index current with the workspace's files.

use lb_rs::Uuid;
use lb_rs::model::file::File;

use super::Link;
use crate::file_cache::{FileCache, FilesExt as _};
use crate::workspace::Workspace;

/// Whether links are read from `file`.
fn is_note(file: &File) -> bool {
    let named = file.name.rsplit_once('.').map_or("", |(_, ext)| ext);
    file.is_document() && named.eq_ignore_ascii_case("md")
}

impl Workspace {
    /// Puts `files` in place of the file cache.
    pub(crate) fn replace_files(&mut self, files: FileCache) {
        *self.files.write().unwrap() = files;
        let files = self.files.read().unwrap();
        let mut links = self.links.write().unwrap();

        links.refresh(&*files);

        for note in files.all_files().filter(|f| is_note(f)) {
            let version = note.last_modified;
            let reader = &mut self.link_reader;
            if links.version(note.id) != Some(version)
                && !reader.reading(note.id, version)
                && !reader.failed_lately(note.id)
            {
                reader.read(note.id, version);
            }
        }
    }

    /// Reads the links of every note for the first time.
    pub(crate) fn read_all_links(&mut self) {
        for note in self
            .files
            .read()
            .unwrap()
            .all_files()
            .filter(|f| is_note(f))
        {
            self.link_reader.read(note.id, note.last_modified);
        }
    }

    /// The document `id` was written; its links may have changed.
    pub(crate) fn links_follow_write(&mut self, id: Uuid) {
        if let Some(note) = self
            .files
            .read()
            .unwrap()
            .get_by_id(id)
            .filter(|f| is_note(f))
        {
            self.link_reader.read(id, note.last_modified);
        }
    }

    /// Takes in what was read since the last frame.
    pub(crate) fn process_links(&mut self) {
        let reads = self.link_reader.finished().into_iter();
        let reads: Vec<_> = reads
            .filter_map(|read| Some((read.note, read.version, read.links?)))
            .collect();
        self.index_links(reads);
    }

    /// Records the links read from notes: each a note, its version, and
    /// its links.
    fn index_links(&mut self, reads: Vec<(Uuid, u64, Vec<Link>)>) {
        if reads.is_empty() {
            return;
        }
        let files = self.files.read().unwrap();
        let mut index = self.links.write().unwrap();
        for (note, version, links) in reads {
            if files.get_by_id(note).is_some() {
                index.set(&*files, note, version, links);
            }
        }
    }
}
