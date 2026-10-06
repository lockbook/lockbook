//! Keeps the link index current with the workspace's files and holds the
//! upkeep that waits on the user.

use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::sync::RwLock;
use std::sync::mpsc::{self, Receiver, Sender};

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::access_info::UserAccessMode;
use lb_rs::model::errors::{LbErr, LbErrKind};
use lb_rs::model::file::File;
use lb_rs::model::svg::buffer::Buffer;

use super::index::shape;
use super::upkeep::{self, Stray};
use super::{Link, LinkIndex, extract};
use crate::file_cache::{FileCache, FilesExt as _};
use crate::workspace::Workspace;

/// Link upkeep that waits on the user.
pub struct Upkeep {
    /// Pasted images that lost a link; those nothing links to are offered
    /// for deletion.
    orphans: Vec<Uuid>,
    /// Notes with a link to a file that is gone, and the file's name.
    lost: Vec<(Uuid, String)>,
    /// Strays whose mend is being written; the notice leaves them out.
    mending: Vec<Stray>,
    /// Orphans to delete once every read asked for is in.
    deleting: Vec<Uuid>,
    /// What came of mends and deletes, which are written off the UI thread.
    done: (Sender<Done>, Receiver<Done>),
    /// Whether every note has been read once; there is nothing to say before.
    built: bool,
    notice: Option<Notice>,
    /// When a notice, and a row of one, was last acted on, in egui's time.
    pub(crate) acted: f64,
    pub(crate) row_acted: f64,
    /// Whether the notice lists what it is about.
    pub(crate) listing: bool,
}

impl Default for Upkeep {
    fn default() -> Self {
        Self {
            orphans: vec![],
            lost: vec![],
            mending: vec![],
            deleting: vec![],
            done: mpsc::channel(),
            built: false,
            notice: None,
            acted: f64::NEG_INFINITY,
            row_acted: f64::NEG_INFINITY,
            listing: false,
        }
    }
}

impl Upkeep {
    /// Remembers the pasted images among `dropped`, files that a note
    /// stopped linking to.
    fn may_be_orphans(&mut self, files: &FileCache, dropped: Vec<Uuid>) {
        for file in dropped {
            if upkeep::pasted_image(files, file) && !self.orphans.contains(&file) {
                self.orphans.push(file);
            }
        }
    }
}

/// What link upkeep has for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// Links a change took from their files.
    Strays(Vec<Stray>),
    /// Pasted images nothing links to any more.
    Orphans(Vec<Uuid>),
    /// Notes with a link to a file that is gone, and the file's name.
    Lost(Vec<(Uuid, String)>),
}

/// What came of work done off the UI thread.
enum Done {
    /// `strays`, all in the note named `name`, were to be mended.
    Mend { name: String, strays: Vec<Stray>, outcome: Result<Option<Vec<Link>>, Unmended> },
    /// Orphans were deleted, but for what is said here.
    Deleted(Vec<String>),
}

/// Why a note's links weren't mended.
enum Unmended {
    /// Its text can't be rewritten to reach the files.
    Unwritable,
    /// The tree changed shape under the mends, which were worked out for
    /// the one before.
    Stale,
    /// Reading or writing it failed.
    Failed(String),
}

/// Whether links are read from `file`.
fn is_note(file: &File) -> bool {
    has_ext(file, "md")
}

fn has_ext(file: &File, ext: &str) -> bool {
    let named = file.name.rsplit_once('.').map_or("", |(_, ext)| ext);
    file.is_document() && named.eq_ignore_ascii_case(ext)
}

impl Workspace {
    /// Puts `files` in place of the file cache.
    pub(crate) fn replace_files(&mut self, files: FileCache) {
        let old = mem::replace(&mut *self.files.write().unwrap(), files);
        {
            let files = self.files.read().unwrap();
            let mut links = self.links.write().unwrap();

            let gone = links.refresh(&*files);
            self.link_upkeep.may_be_orphans(&files, gone.dropped);
            for (note, file) in gone.lost {
                if let Some(file) = old.get_by_id(file) {
                    self.link_upkeep.lost.push((note, file.name.clone()));
                }
            }
            self.link_upkeep
                .lost
                .retain(|(note, _)| files.get_by_id(*note).is_some());
            self.settle_left(&files, &mut links);

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
        self.renotice();
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

    /// Takes in what was read and written since the last frame.
    pub(crate) fn process_links(&mut self) {
        let reads = self.link_reader.finished().into_iter();
        let reads: Vec<_> = reads
            .filter_map(|read| Some((read.note, read.version, read.links?)))
            .collect();
        let mut changed = !reads.is_empty();
        self.index_links(reads);

        while let Ok(done) = self.link_upkeep.done.1.try_recv() {
            changed = true;
            let (name, strays, outcome) = match done {
                Done::Mend { name, strays, outcome } => (name, strays, outcome),
                Done::Deleted(said) => {
                    self.out.failure_messages.extend(said);
                    continue;
                }
            };
            self.link_upkeep.mending.retain(|m| !strays.contains(m));
            let Some(note) = strays.first().map(|s| s.note) else { continue };
            let failure = match outcome {
                Ok(Some(links)) => {
                    // the index has it at once, and no read from before it stands
                    let version = self.links.read().unwrap().version(note);
                    self.index_links(vec![(note, version.unwrap_or_default(), links)]);
                    self.link_reader.read(note, version.unwrap_or_default());
                    continue;
                }
                Ok(None) | Err(Unmended::Stale) => continue,
                Err(Unmended::Unwritable) => {
                    self.leave_links(&strays);
                    format!("Couldn't update links in {name}")
                }
                Err(Unmended::Failed(why)) => format!("Couldn't update links in {name}: {why}"),
            };
            self.out.failure_messages.push(failure);
        }

        let idle = self.link_reader.idle();
        if idle && !self.link_upkeep.deleting.is_empty() {
            self.delete_orphans_now();
        }
        let built = idle && !self.link_upkeep.built;
        self.link_upkeep.built |= idle;
        if changed || built {
            self.renotice();
        }
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
                let dropped = index.set(&*files, note, version, links);
                self.link_upkeep.may_be_orphans(&files, dropped);
            }
        }
        self.settle_left(&files, &mut index);
    }

    /// Keeps links left as they are settled for as long as they are broken,
    /// and forgets the rest of them.
    fn settle_left(&self, files: &FileCache, index: &mut LinkIndex) {
        let mut left = self.cfg.links_left();
        let before = left.len();
        left.retain(|(note, dest)| {
            let mut written = index
                .outbound(*note)
                .iter()
                .filter(|l| l.link.dest == *dest);
            let broken = index.version(*note).is_none() || written.any(|l| l.target.is_none());
            files.get_by_id(*note).is_some() && broken
        });
        for (note, dest) in &left {
            index.settle(*note, dest);
        }
        if left.len() != before {
            self.cfg.set_links_left(left);
        }
    }

    /// Works out what there is to say, something it rests on having changed.
    fn renotice(&mut self) {
        let upkeep = &self.link_upkeep;
        if !upkeep.built {
            return;
        }
        let files = self.files.read().unwrap();
        let index = self.links.read().unwrap();

        let mut strays = upkeep::strays(&*files, &index);
        strays.retain(|s| !upkeep.mending.contains(s));

        // the images this account pasted: anyone else may link to theirs
        // from where this account can't see
        let mine = |file: &Uuid| {
            let file = files.get_by_id(*file);
            file.is_some_and(|f| f.last_modified_by == self.account.username)
        };
        let orphaned = |file: &Uuid| upkeep::orphaned(&*files, &index, *file) && mine(file);
        let mut orphans: Vec<Uuid> = upkeep.orphans.iter().copied().filter(orphaned).collect();
        orphans.sort();
        // a note not yet read may be what links to them
        let unread = |f: &File| is_note(f) && index.version(f.id).is_none();
        let read = orphans.is_empty() || !files.all_files().any(unread);

        let notice = if !strays.is_empty() {
            Some(Notice::Strays(strays))
        } else if !orphans.is_empty() && read {
            Some(Notice::Orphans(orphans))
        } else {
            (!upkeep.lost.is_empty()).then(|| Notice::Lost(upkeep.lost.clone()))
        };
        drop((files, index));
        self.link_upkeep.notice = notice;
    }

    /// What link upkeep has for the user, once every note's links are read.
    pub fn link_notice(&self) -> Option<&Notice> {
        self.link_upkeep.notice.as_ref()
    }

    /// Whether a link in `note` can be rewritten here.
    pub fn can_mend(&self, note: Uuid) -> bool {
        let files = self.files.read().unwrap();
        files.get_by_id(note).is_some() && files.access(note, &self.account) != UserAccessMode::Read
    }

    /// Writes the mend of each of `strays` that has one, in the notes that
    /// can be written.
    pub fn mend_links(&mut self, mut strays: Vec<Stray>) {
        strays.retain(|s| s.mend.is_some() && self.can_mend(s.note));
        let mut notes: Vec<Uuid> = strays.iter().map(|s| s.note).collect();
        notes.sort();
        notes.dedup();
        let names: Vec<String> = {
            let files = self.files.read().unwrap();
            notes.iter().map(|n| files.path(*n)).collect()
        };
        self.link_upkeep.mending.extend(strays.iter().cloned());
        self.renotice();

        let (core, files, ctx) = (self.core.clone(), self.files.clone(), self.ctx.clone());
        let tree = shape(&*files.read().unwrap());
        let done = self.link_upkeep.done.0.clone();
        lb_rs::spawn!({
            for (note, name) in notes.into_iter().zip(names) {
                let strays: Vec<Stray> =
                    strays.iter().filter(|s| s.note == note).cloned().collect();
                // a note the parser chokes on doesn't hold the rest back
                let mend = AssertUnwindSafe(|| mend_note(&core, &files, tree, note, &strays));
                let unread = |_| Err(Unmended::Failed("it couldn't be read".into()));
                let outcome = panic::catch_unwind(mend).unwrap_or_else(unread);
                let _ = done.send(Done::Mend { name, strays, outcome });
                ctx.request_repaint();
            }
        });
    }

    /// Takes `strays` to mean what they reach now. Those that reach nothing
    /// are left so after a restart too, when they would be read as new.
    pub fn leave_links(&mut self, strays: &[Stray]) {
        let mut left = self.cfg.links_left();
        let mut index = self.links.write().unwrap();
        for stray in strays {
            index.settle(stray.note, &stray.dest);
            let mut written = index.outbound(stray.note).iter();
            if written.any(|l| l.link.dest == stray.dest && l.target.is_none()) {
                left.push((stray.note, stray.dest.clone()));
            }
        }
        drop(index);
        left.sort();
        left.dedup();
        self.cfg.set_links_left(left);
        self.renotice();
    }

    /// Deletes those of `images` that nothing links to, once what is being
    /// read is known.
    pub fn delete_orphans(&mut self, images: Vec<Uuid>) {
        self.link_upkeep.orphans.retain(|f| !images.contains(f));
        self.link_upkeep.deleting.extend(images);
        self.renotice();
    }

    fn delete_orphans_now(&mut self) {
        let images = mem::take(&mut self.link_upkeep.deleting);
        // a drawing keeps the images pasted into it beside it too, by id
        let images: Vec<(Uuid, String, Vec<Uuid>)> = {
            let files = self.files.read().unwrap();
            let index = self.links.read().unwrap();
            let orphaned = |image: &Uuid| upkeep::orphaned(&*files, &index, *image);
            let beside = |image: Uuid| {
                let imports = files.get_by_id(image).map(|f| f.parent);
                let folder = imports.and_then(|id| files.get_by_id(id)).map(|f| f.parent);
                let beside = folder.map(|id| files.children(id)).unwrap_or_default();
                let drawings = beside.into_iter().filter(|f| has_ext(f, "svg"));
                drawings.map(|f| f.id).collect()
            };
            let images = images.into_iter().filter(orphaned);
            images
                .map(|image| (image, files.path(image), beside(image)))
                .collect()
        };

        let (core, ctx) = (self.core.clone(), self.ctx.clone());
        let done = self.link_upkeep.done.0.clone();
        lb_rs::spawn!({
            let mut said = vec![];
            for (image, name, drawings) in images {
                if drawings.iter().any(|d| drawing_shows(&core, *d, image)) {
                    said.push(format!("Kept {name}: a drawing beside it shows it"));
                } else if let Err(err) = core.delete_file(&image) {
                    said.push(format!("Couldn't delete {name}: {:?}", err.kind));
                }
            }
            let _ = done.send(Done::Deleted(said));
            ctx.request_repaint();
        });
    }

    /// Acts on the notice at hand: mends what it can and leaves the rest,
    /// or deletes what nothing links to.
    pub fn accept_link_notice(&mut self) {
        match self.link_upkeep.notice.take() {
            Some(Notice::Strays(strays)) => {
                let mendable = |s: &Stray| s.mend.is_some() && self.can_mend(s.note);
                let (mend, leave): (Vec<Stray>, Vec<Stray>) =
                    strays.into_iter().partition(mendable);
                self.leave_links(&leave);
                self.mend_links(mend);
            }
            Some(Notice::Orphans(orphans)) => self.delete_orphans(orphans),
            Some(Notice::Lost(_)) | None => self.renotice(),
        }
        self.acted_on_link_notice();
    }

    /// Lets the notice at hand go without acting on it.
    pub fn dismiss_link_notice(&mut self) {
        match self.link_upkeep.notice.take() {
            Some(Notice::Strays(strays)) => self.leave_links(&strays),
            Some(Notice::Orphans(orphans)) => {
                self.link_upkeep.orphans.retain(|f| !orphans.contains(f));
                self.renotice();
            }
            Some(Notice::Lost(_)) => {
                self.link_upkeep.lost.clear();
                self.renotice();
            }
            None => {}
        }
        self.acted_on_link_notice();
    }

    fn acted_on_link_notice(&mut self) {
        self.link_upkeep.listing = false;
        self.link_upkeep.acted = self.ctx.input(|i| i.time);
    }
}

/// Rewrites the stray links in `note` on disk and returns its links as
/// written, `None` if it had none of them to mend. The strays' mends were
/// worked out in a tree of shape `tree`. An open tab picks the write up as
/// it would a sync.
fn mend_note(
    core: &Lb, files: &RwLock<FileCache>, tree: u64, note: Uuid, strays: &[Stray],
) -> Result<Option<Vec<Link>>, Unmended> {
    let failed = |err: LbErr| Unmended::Failed(format!("{:?}", err.kind));
    loop {
        let (hmac, bytes) = core.read_document_with_hmac(note, false).map_err(failed)?;
        let text = String::from_utf8(bytes).map_err(|_| Unmended::Unwritable)?;
        let mended = {
            let files = files.read().unwrap();
            let unmended = |_| match shape(&*files) == tree {
                true => Unmended::Unwritable,
                false => Unmended::Stale,
            };
            upkeep::mend(&*files, &text, strays).map_err(unmended)?
        };
        let Some(mended) = mended else { return Ok(None) };
        let links = extract(&mended);
        match core.safe_write(note, hmac, mended.into_bytes(), None) {
            Ok(_) => return Ok(Some(links)),
            Err(err) if err.kind == LbErrKind::ReReadRequired => continue,
            Err(err) => return Err(failed(err)),
        }
    }
}

/// Whether `drawing` shows `image`, or can't be read to tell.
fn drawing_shows(core: &Lb, drawing: Uuid, image: Uuid) -> bool {
    let Ok(bytes) = core.read_document(drawing, false) else { return true };
    let Ok(text) = String::from_utf8(bytes) else { return false };
    let shows = || {
        let images = Buffer::new(&text).weak_images;
        images.values().any(|shown| shown.href == image)
    };
    panic::catch_unwind(shows).unwrap_or(true)
}
