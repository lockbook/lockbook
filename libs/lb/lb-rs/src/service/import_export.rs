//! Import plans what bringing folders and files in would do, then writes exactly that plan.
//! Export copies Lockbook files out to disk.

use crate::Lb;
use crate::io::server::Server;
use crate::model::ValidationFailure;
use crate::model::api::METADATA_FEE;
use crate::model::clock::get_time;
use crate::model::crypto::AESKey;
use crate::model::errors::{LbErr, LbErrKind, LbResult};
use crate::model::file::File;
use crate::model::file_like::FileLike;
use crate::model::file_metadata::FileType;
use crate::model::symkey;
use crate::model::tree_like::TreeLike;
use crate::service::activity;
use crate::service::documents::{compress_encrypt_document, document_hmac};
use crate::service::events::Actor;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::collections::hash_map::Entry as Slot;
use std::collections::{HashMap, HashSet};
use std::fs::{self, Metadata, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::sync::Semaphore;
use uuid::Uuid;

// File descriptors are process-wide. Share the limit across nested folders and
// simultaneous exports, and only acquire permits for documents (never parents).
static EXPORT_DOCUMENTS: Semaphore = Semaphore::const_new(16);

pub enum ImportStatus {
    CalculatedTotal(usize),
    StartingItem(String),
    FinishedItem(File),
}

/// What an import does with a file or folder already under the destination by the same name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Existing {
    /// Leave it alone and bring the import in beside it, named `name-1`.
    Copy,
    /// Merge into it: replace documents whose contents differ and keep the rest. Only for an
    /// import whose plan someone saw and went ahead with.
    Update,
}

/// What importing some folders and files would do, decided before anything is written.
#[derive(Clone, Debug)]
pub struct ImportPlan {
    pub dest: Uuid,
    /// Depth-first: every folder comes before what's inside it.
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub disk_path: PathBuf,
    pub file_type: EntryType,
    pub action: Action,
    /// Skipped by the user's choice. `action` stays as the walk found it, so changing the
    /// choice doesn't re-scan files.
    pub skip: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryType {
    Folder,
    /// Its size on disk, its size once compressed and encrypted, and how much writing it grows
    /// the account's usage: a new document's stored bytes and file fee, or what a replaced one
    /// grows by.
    Document {
        disk_bytes: u64,
        stored_bytes: u64,
        added_bytes: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Create,
    Replace(Uuid),
    /// Already in Lockbook: a document with the same contents, or a folder that exists.
    Noop(Uuid),
    Ignore(IgnoreReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgnoreReason {
    /// What a file browser hides: a name starting with a dot, or the system's hidden mark.
    Hidden,
    Unreadable,
    /// A symbolic link, pipe, socket or device.
    NotAFile,
}

/// A count of documents and how much they grow the account's usage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocTotals {
    pub files: usize,
    pub added_bytes: u64,
}

impl DocTotals {
    fn add(&mut self, added_bytes: u64) {
        self.files += 1;
        self.added_bytes += added_bytes;
    }
}

/// An import plan's totals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub new: usize,
    pub changed: usize,
    /// Documents already here with the same contents.
    pub same: usize,
    /// The written documents' size on disk.
    pub disk_bytes: u64,
    /// What the writes take up once stored, file fees included: a replaced document counts whole.
    pub stored_bytes: u64,
    /// How much the writes grow the account's usage.
    pub added_bytes: u64,
    pub skipped: DocTotals,
    pub hidden: usize,
    pub unreadable: usize,
    pub not_a_file: usize,
}

impl Entry {
    pub fn name(&self) -> String {
        self.disk_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The import creates this folder, or creates or replaces this document.
    pub fn writes(&self) -> bool {
        matches!(self.action, Action::Create | Action::Replace(_)) && !self.skip
    }
}

impl<S: Server> Lb<S> {
    /// Creates a folder, or a document holding `content` from the transaction that creates it.
    /// When `name` is taken, it becomes `name-1`, `name-2` and so on until one is free.
    /// It never writes over anything; replacing a document is [`Action::Replace`]'s job.
    async fn create_imported(
        &self, name: &str, parent: Uuid, content: Option<&[u8]>,
    ) -> LbResult<File> {
        let id = Uuid::new_v4();
        let key = symkey::generate_key();
        let document = match content {
            Some(content) => {
                let (hmac, encrypted) = compress_encrypt_document(&key, content)?;
                let pending = self.docs.insert_pending(id, hmac, &encrypted).await?;
                Some((pending, hmac, encrypted.value.len()))
            }
            None => None,
        };
        let file_type = if document.is_some() { FileType::Document } else { FileType::Folder };

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        let content = match document {
            Some((pending, hmac, size)) => {
                pending.promote().await?;
                Some((hmac, size))
            }
            None => None,
        };
        let mut free_name = name.to_string();
        for tries in 1.. {
            match tree.create(id, key, &parent, &free_name, file_type, &self.keychain) {
                Err(err)
                    if matches!(
                        err.kind,
                        LbErrKind::Validation(ValidationFailure::PathConflict(_))
                    ) =>
                {
                    free_name = format!("{name}-{tries}");
                }
                result => {
                    result?;
                    break;
                }
            }
        }
        if let Some((hmac, size)) = content {
            tree.overwrite_document_hmac(&id, Some(hmac), Some(size), &self.keychain)?;
        }
        let file = tree.decrypt(&self.keychain, &id, &db.pub_key_lookup)?;
        tx.end();

        self.events.meta_changed(Actor::User(None));
        if content.is_some() {
            self.events.doc_written(id, Actor::User(None));
            self.add_doc_event(activity::DocEvent::Write(id, get_time().0))
                .await?;
        }
        Ok(file)
    }

    /// Whether document `id` holds `content`, judged from its metadata so nothing is fetched.
    async fn holds(&self, id: Uuid, content: &[u8]) -> LbResult<bool> {
        let (hmac, key) = {
            let tx = self.ro_tx().await;
            let db = tx.db();
            let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
            let hmac = tree.find(&id)?.document_hmac().copied();
            (hmac, tree.decrypt_key(&id, &self.keychain)?)
        };
        Ok(match hmac {
            Some(hmac) => document_hmac(&key, content)? == hmac,
            None => content.is_empty(),
        })
    }
}

/// What the server counts for a document holding `data`: compressed, then encrypted. Exact for
/// any key, since compression comes first and the nonce and tag have fixed lengths.
fn stored_size(key: &AESKey, data: &[u8]) -> LbResult<u64> {
    Ok(compress_encrypt_document(key, data)?.1.value.len() as u64)
}

/// What a file browser hides: a name starting with a dot, or the system's hidden mark.
fn hidden(name: &str, meta: &Metadata) -> bool {
    if name.starts_with('.') {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        const UF_HIDDEN: u32 = 0x8000;
        meta.st_flags() & UF_HIDDEN != 0
    }
    #[cfg(target_os = "ios")]
    {
        use std::os::ios::fs::MetadataExt;
        const UF_HIDDEN: u32 = 0x8000;
        meta.st_flags() & UF_HIDDEN != 0
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios", windows)))]
    {
        let _ = meta;
        false
    }
}

impl<S: Server> Lb<S> {
    /// `parent` is the Lockbook folder `disk_path` would land in, when that folder exists.
    async fn plan_entry(
        &self, disk_path: &Path, parent: Option<Uuid>, top: bool, key: &AESKey,
        children: &mut HashMap<Uuid, Vec<File>>,
    ) -> LbResult<(EntryType, Action)> {
        let unread = EntryType::Document { disk_bytes: 0, stored_bytes: 0, added_bytes: 0 };

        // 1. Look at the entry itself. Links are never followed, so the walk stays inside what
        //    was picked.
        let Ok(meta) = fs::symlink_metadata(disk_path) else {
            return Ok((unread, Action::Ignore(IgnoreReason::Unreadable)));
        };
        let file_type = if meta.is_dir() { EntryType::Folder } else { unread };
        let Some(name) = disk_path.file_name().and_then(|n| n.to_str()) else {
            return Ok((file_type, Action::Ignore(IgnoreReason::Unreadable)));
        };

        // 2. Leave out what a file browser hides, unless it was picked by name.
        if !top && hidden(name, &meta) {
            return Ok((file_type, Action::Ignore(IgnoreReason::Hidden)));
        }

        // 3. Find what's already in Lockbook by this name, listing each folder only once.
        let existing = match parent {
            Some(parent) => {
                let listed = match children.entry(parent) {
                    Slot::Occupied(listed) => listed.into_mut(),
                    Slot::Vacant(slot) => slot.insert(self.get_children(&parent).await?),
                };
                listed.iter().find(|f| f.name == name).cloned()
            }
            None => None,
        };

        // 4. A folder merges into the folder already there, or is created.
        if meta.is_dir() {
            let action = match existing.filter(|f| f.is_folder()) {
                Some(f) => Action::Noop(f.id),
                None => Action::Create,
            };
            return Ok((file_type, action));
        }

        // 5. Of everything else, only regular files that can be read come in.
        if !meta.is_file() {
            return Ok((file_type, Action::Ignore(IgnoreReason::NotAFile)));
        }
        let Ok(data) = fs::read(disk_path) else {
            return Ok((file_type, Action::Ignore(IgnoreReason::Unreadable)));
        };
        let disk_bytes = data.len() as u64;
        let existing = existing.filter(|f| f.is_document());
        // `size_bytes` counts the file fee on top of the stored document.
        let held = |f: &File| f.size_bytes.saturating_sub(METADATA_FEE);

        // 6. A document with the same contents is already here and costs nothing.
        if let Some(f) = &existing {
            if self.holds(f.id, &data).await? {
                let (stored_bytes, added_bytes) = (held(f), 0);
                return Ok((
                    EntryType::Document { disk_bytes, stored_bytes, added_bytes },
                    Action::Noop(f.id),
                ));
            }
        }

        // 7. Otherwise a replacement grows usage by the difference, and a new document by its
        //    whole stored size plus the file fee.
        let stored_bytes = stored_size(key, &data)?;
        Ok(match existing {
            Some(f) => {
                let added_bytes = stored_bytes.saturating_sub(held(&f));
                (
                    EntryType::Document { disk_bytes, stored_bytes, added_bytes },
                    Action::Replace(f.id),
                )
            }
            None => {
                let added_bytes = stored_bytes + METADATA_FEE;
                (EntryType::Document { disk_bytes, stored_bytes, added_bytes }, Action::Create)
            }
        })
    }

    /// Walks `sources` and, for [`Existing::Update`], compares each file with what's already
    /// under `dest`, writing nothing.
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn plan_import(
        &self, sources: &[PathBuf], dest: Uuid, existing: Existing,
    ) -> LbResult<ImportPlan> {
        // 1. The destination must be a folder.
        let parent = self.get_file_by_id(dest).await?;
        if !parent.is_folder() {
            Err(LbErrKind::Validation(ValidationFailure::NonFolderWithChildren(dest)))?;
        }

        // 2. One throwaway key measures every document.
        let key = symkey::generate_key();
        let mut children = HashMap::new();
        let mut entries = Vec::new();

        // 3. Start from the picked sources, dropping any inside another, since it's already part
        //    of it. Only an update compares against `dest`; a copy matches nothing and creates
        //    everything.
        let mut stack: Vec<(PathBuf, Option<Uuid>, bool)> = sources
            .iter()
            .rev()
            .filter(|s| !sources.iter().any(|o| o != *s && s.starts_with(o)))
            .map(|s| (s.clone(), (existing == Existing::Update).then_some(dest), true))
            .collect();

        // 4. Plan each entry, then queue a folder's contents to come right after it, in name
        //    order. They're compared against the Lockbook folder only when it already exists.
        while let Some((disk_path, parent, top)) = stack.pop() {
            let (file_type, mut action) = self
                .plan_entry(&disk_path, parent, top, &key, &mut children)
                .await?;
            if file_type == EntryType::Folder && !matches!(action, Action::Ignore(_)) {
                let listed: std::io::Result<Vec<PathBuf>> = fs::read_dir(&disk_path)
                    .and_then(|dir| dir.map(|child| Ok(child?.path())).collect());
                match listed {
                    Ok(mut inside) => {
                        let here = match action {
                            Action::Noop(id) => Some(id),
                            _ => None,
                        };
                        inside.sort();
                        stack.extend(inside.into_iter().rev().map(|c| (c, here, false)));
                    }
                    // A folder that can't be listed can't be imported.
                    Err(_) => action = Action::Ignore(IgnoreReason::Unreadable),
                }
            }
            entries.push(Entry { disk_path, file_type, action, skip: false });
        }
        Ok(ImportPlan { dest, entries })
    }
}

impl ImportPlan {
    /// The plan's totals, counted from its entries.
    pub fn summary(&self) -> ImportSummary {
        let mut s = ImportSummary::default();
        for e in &self.entries {
            match (e.action, e.file_type) {
                (Action::Ignore(IgnoreReason::Hidden), _) => s.hidden += 1,
                (Action::Ignore(IgnoreReason::Unreadable), _) => s.unreadable += 1,
                (Action::Ignore(IgnoreReason::NotAFile), _) => s.not_a_file += 1,
                (Action::Noop(_), EntryType::Document { .. }) => s.same += 1,
                (Action::Noop(_), EntryType::Folder) => {}
                (_, EntryType::Folder) => {
                    s.stored_bytes += METADATA_FEE;
                    s.added_bytes += METADATA_FEE;
                }
                (_, EntryType::Document { added_bytes, .. }) if e.skip => {
                    s.skipped.add(added_bytes)
                }
                (action, EntryType::Document { disk_bytes, stored_bytes, added_bytes, .. }) => {
                    if action == Action::Create {
                        s.new += 1;
                        s.stored_bytes += stored_bytes + METADATA_FEE;
                    } else {
                        s.changed += 1;
                        s.stored_bytes += stored_bytes;
                    }
                    s.disk_bytes += disk_bytes;
                    s.added_bytes += added_bytes;
                }
            }
        }
        s
    }
}

impl<S: Server> Lb<S> {
    /// Writes one planned entry under `parent`, reading its file only now.
    async fn import_entry(&self, e: &Entry, parent: Uuid) -> LbResult<File> {
        let name = e
            .disk_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(LbErrKind::DiskPathInvalid)?;
        if e.file_type == EntryType::Folder {
            return self.create_imported(name, parent, None).await;
        }
        let content = fs::read(&e.disk_path).map_err(LbErr::from)?;
        match e.action {
            Action::Replace(id) => {
                self.write_document(id, &content).await?;
                self.get_file_by_id(id).await
            }
            _ => self.create_imported(name, parent, Some(&content)).await,
        }
    }

    /// Writes what `plan` says, one file at a time, then fails naming every entry it couldn't
    /// write. An entry inside a folder that wasn't created fails with it.
    #[instrument(level = "debug", skip(self, plan, update_status), err(Debug))]
    pub async fn import_planned<F: Fn(ImportStatus)>(
        &self, plan: &ImportPlan, update_status: &F,
    ) -> LbResult<()> {
        let total = plan.entries.iter().filter(|e| e.writes()).count();
        update_status(ImportStatus::CalculatedTotal(total));

        let folders: HashSet<&Path> = plan
            .entries
            .iter()
            .filter(|e| e.file_type == EntryType::Folder)
            .map(|e| e.disk_path.as_path())
            .collect();
        let mut ids: HashMap<&Path, Uuid> = HashMap::new();
        let mut failed = Vec::new();
        for e in &plan.entries {
            if let (Action::Noop(id), EntryType::Folder) = (e.action, e.file_type) {
                ids.insert(&e.disk_path, id);
            }
            if !e.writes() {
                continue;
            }
            let parent = match e.disk_path.parent().filter(|p| folders.contains(p)) {
                Some(folder) => ids.get(folder).copied(),
                None => Some(plan.dest),
            };
            update_status(ImportStatus::StartingItem(e.disk_path.display().to_string()));
            let result = match parent {
                Some(parent) => self.import_entry(e, parent).await,
                None => Err(LbErrKind::FileParentNonexistent.into()),
            };
            match result {
                Ok(file) => {
                    if e.file_type == EntryType::Folder {
                        ids.insert(&e.disk_path, file.id);
                    }
                    update_status(ImportStatus::FinishedItem(file));
                }
                Err(err) => {
                    error!(?err, path = %e.disk_path.display(), "import failed");
                    failed.push(e.disk_path.clone());
                }
            }
        }
        if failed.is_empty() {
            Ok(())
        } else {
            Err(LbErrKind::ImportFailed { paths: failed, total }.into())
        }
    }

    /// Imports `sources` into `dest`, beside anything already there by the same name. Files that
    /// can't be read, and sources that are links or otherwise not files, fail by name in
    /// [`LbErrKind::ImportFailed`] along with any that can't be written.
    #[instrument(level = "debug", skip(self, update_status), err(Debug))]
    pub async fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> LbResult<()> {
        let plan = self.plan_import(sources, dest, Existing::Copy).await?;
        let mut failed: Vec<PathBuf> = plan
            .entries
            .iter()
            .filter(|e| match e.action {
                Action::Ignore(IgnoreReason::Unreadable) => true,
                Action::Ignore(IgnoreReason::NotAFile) => sources.contains(&e.disk_path),
                _ => false,
            })
            .map(|e| e.disk_path.clone())
            .collect();
        let total = failed.len() + plan.entries.iter().filter(|e| e.writes()).count();
        if let Err(err) = self.import_planned(&plan, update_status).await {
            let LbErrKind::ImportFailed { paths, .. } = &err.kind else {
                return Err(err);
            };
            failed.extend(paths.iter().cloned());
        }
        if failed.is_empty() {
            Ok(())
        } else {
            Err(LbErrKind::ImportFailed { paths: failed, total }.into())
        }
    }
}

impl<S: Server> Lb<S> {
    #[instrument(level = "debug", skip(self, update_status), err(Debug))]
    pub async fn export_file<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, dest: PathBuf, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()> {
        if dest.is_file() {
            return Err(LbErrKind::DiskPathInvalid.into());
        }

        self.export_file_recursively(id, &dest, edit, update_status)
            .await
    }

    pub async fn export_file_recursively<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, disk_path: &Path, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()> {
        let file = self.get_file_by_id(id).await?;

        let new_dest = disk_path.join(&file.name);

        if let Some(update_status) = update_status {
            update_status(ExportFileInfo {
                disk_path: disk_path.to_path_buf(),
                lockbook_path: self.get_path_by_id(file.id).await?,
            });
        }

        match file.file_type {
            FileType::Document => {
                let _permit = EXPORT_DOCUMENTS
                    .acquire()
                    .await
                    .expect("export semaphore is never closed");
                let content = self.read_document(file.id, true).await?;

                // Do not hold an output file open while awaiting document I/O.
                let mut disk_file = if edit {
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(new_dest)
                } else {
                    OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(new_dest)
                }
                .map_err(LbErr::from)?;

                disk_file.write_all(&content).map_err(LbErr::from)?;
            }
            FileType::Folder => {
                fs::create_dir(new_dest.clone()).map_err(LbErr::from)?;
                let export_file_futures = FuturesUnordered::new();

                for child in self.get_children(&file.id).await? {
                    let lb = self.clone();
                    let new_dest = &new_dest;

                    export_file_futures.push(async move {
                        lb.export_file_recursively(child.id, new_dest, edit, update_status)
                            .await
                    });
                }

                export_file_futures
                    .collect::<Vec<LbResult<()>>>()
                    .await
                    .into_iter()
                    .collect::<LbResult<()>>()?;
            }
            FileType::Link { target } => {
                let export_file_futures = FuturesUnordered::new();
                let lb = self.clone();

                export_file_futures.push(async move {
                    lb.export_file_recursively(target, disk_path, edit, update_status)
                        .await
                });

                export_file_futures
                    .collect::<Vec<LbResult<()>>>()
                    .await
                    .into_iter()
                    .collect::<LbResult<()>>()?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ExportFileInfo {
    pub disk_path: PathBuf,
    pub lockbook_path: String,
}
