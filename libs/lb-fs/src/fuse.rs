//! A FUSE mount of the desktop app's account, for Linux file managers.
//!
//! Shares the app's data directory under lb-rs's multi-process locks. Each open file is a buffer
//! read with its content version; closing it writes back only if that version is still current,
//! and otherwise saves the buffer as a conflicted copy beside the file.

use fuser::{
    Config as FuseConfig, Errno, FileAttr, FileHandle, FileType as Kind, Filesystem, FopenFlags,
    Generation, INodeNo, InitFlags, KernelConfig, LockOwner, MountOption, Notifier, OpenAccMode,
    OpenFlags, RenameFlags, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request, Session, TimeOrNow, WriteFlags,
};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::ValidationFailure;
use lb_rs::model::errors::{LbErr, LbErrKind};
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::{DocumentHmac, FileType};
use lb_rs::service::events::{Actor, Event};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tracing::{info, warn};

/// How long the kernel trusts names and attributes; sync changes also invalidate them directly.
const TTL: Duration = Duration::from_secs(1);

pub struct LbFuse {
    lb: Lb,
    state: Arc<Mutex<State>>,
    uid: u32,
    gid: u32,
}

#[derive(Default)]
struct State {
    ids: HashMap<u64, Uuid>,
    inos: HashMap<Uuid, u64>,
    parents: HashMap<u64, u64>,
    next_ino: u64,
    /// The names each directory last showed the kernel, to invalidate what sync changes.
    listed: HashMap<u64, HashMap<String, u64>>,
    /// Plaintext sizes by content version, and the versions sync has to compare against.
    sizes: HashMap<Uuid, (Option<DocumentHmac>, u64)>,
    /// The version each process last read, which its temp-file-and-rename saves are based on.
    served: HashMap<(u32, Uuid), Option<DocumentHmac>>,
    handles: HashMap<u64, Handle>,
    next_fh: u64,
    last_active: Option<Instant>,
}

struct Handle {
    id: Uuid,
    base: Option<DocumentHmac>,
    buf: Vec<u8>,
    pid: u32,
    writable: bool,
    dirty: bool,
}

impl State {
    fn ino(&mut self, id: Uuid, parent: u64) -> u64 {
        let ino = match self.inos.get(&id) {
            Some(ino) => *ino,
            None => {
                self.next_ino += 1;
                let ino = self.next_ino;
                self.ids.insert(ino, id);
                self.inos.insert(id, ino);
                ino
            }
        };
        if ino != INodeNo::ROOT.0 {
            self.parents.insert(ino, parent);
        }
        ino
    }

    fn id(&self, ino: INodeNo) -> Result<Uuid, Errno> {
        self.ids.get(&ino.0).copied().ok_or(Errno::ENOENT)
    }
}

fn errno(err: LbErr) -> Errno {
    match err.kind {
        LbErrKind::FileNonexistent | LbErrKind::FileParentNonexistent => Errno::ENOENT,
        LbErrKind::FileNameContainsSlash | LbErrKind::FileNameEmpty => Errno::EINVAL,
        LbErrKind::FileNameTooLong => Errno::ENAMETOOLONG,
        LbErrKind::FileNotDocument => Errno::EISDIR,
        LbErrKind::InsufficientPermission => Errno::EACCES,
        LbErrKind::Validation(ValidationFailure::PathConflict(_)) => Errno::EEXIST,
        LbErrKind::Validation(ValidationFailure::Cycle(_)) => Errno::EINVAL,
        LbErrKind::Validation(ValidationFailure::FileNameTooLong(_)) => Errno::ENAMETOOLONG,
        LbErrKind::UsageIsOverDataCap | LbErrKind::UsageIsOverFreeTierDataCap => Errno::ENOSPC,
        _ => {
            warn!("unmapped lb error: {err:?}");
            Errno::EIO
        }
    }
}

/// The process behind a request; FUSE reports the calling thread, and GIO saves on a worker.
fn process(req: &Request) -> u32 {
    let tid = req.pid();
    std::fs::read_to_string(format!("/proc/{tid}/status"))
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Tgid:")?.trim().parse().ok())
        })
        .unwrap_or(tid)
}

fn time(ms: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms)
}

/// `notes.md` → `notes (conflicted copy 1731200000).md`
fn conflict_name(name: &str) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => {
            format!("{stem} (conflicted copy {stamp}).{ext}")
        }
        _ => format!("{name} (conflicted copy {stamp})"),
    }
}

impl LbFuse {
    pub fn new(lb: Lb) -> Result<Self, LbErr> {
        let root = lb.get_root()?.id;
        let mut state = State { next_ino: INodeNo::ROOT.0, ..Default::default() };
        state.ids.insert(INodeNo::ROOT.0, root);
        state.inos.insert(root, INodeNo::ROOT.0);
        // SAFETY: getuid and getgid cannot fail.
        let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
        Ok(Self { lb, state: Arc::new(Mutex::new(state)), uid, gid })
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    /// Releases the lock before returning, unlike `self.id(..)` inside a longer expression.
    fn id(&self, ino: INodeNo) -> Result<Uuid, Errno> {
        self.state().id(ino)
    }

    /// Keeps lb-rs on its 3-second sync cadence while someone browses the mount.
    fn touch(&self) {
        let mut state = self.state();
        if state
            .last_active
            .is_none_or(|t| t.elapsed() > Duration::from_secs(5))
        {
            state.last_active = Some(Instant::now());
            drop(state);
            self.lb.app_foregrounded();
        }
    }

    fn attr(&self, file: &File, ino: u64) -> Result<FileAttr, Errno> {
        let (kind, perm, size) = if file.is_folder() {
            (Kind::Directory, 0o755, 0)
        } else {
            (Kind::RegularFile, 0o644, self.size(file.id)?)
        };
        let mtime = time(file.last_modified);
        Ok(FileAttr {
            ino: INodeNo(ino),
            size,
            blocks: size.div_ceil(512),
            atime: mtime,
            mtime,
            ctime: mtime,
            crtime: mtime,
            kind,
            perm,
            nlink: 1,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        })
    }

    /// The plaintext size, from an unsaved buffer if one is open, else cached per content version.
    fn size(&self, id: Uuid) -> Result<u64, Errno> {
        {
            let state = self.state();
            if let Some(h) = state.handles.values().find(|h| h.id == id && h.dirty) {
                return Ok(h.buf.len() as u64);
            }
        }
        let hmac = self.lb.document_hmac(id).map_err(errno)?;
        if let Some((cached, size)) = self.state().sizes.get(&id) {
            if *cached == hmac {
                return Ok(*size);
            }
        }
        let (hmac, doc) = self.lb.read_document_with_hmac(id, false).map_err(errno)?;
        let size = doc.len() as u64;
        self.state().sizes.insert(id, (hmac, size));
        Ok(size)
    }

    fn child(&self, parent: Uuid, name: &OsStr) -> Result<File, Errno> {
        let name = name.to_str().ok_or(Errno::EINVAL)?;
        self.lb
            .get_children(&parent)
            .map_err(errno)?
            .into_iter()
            .find(|f| f.name == name)
            .ok_or(Errno::ENOENT)
    }

    fn entry(&self, parent: INodeNo, file: &File) -> Result<FileAttr, Errno> {
        let ino = {
            let mut state = self.state();
            let ino = state.ino(file.id, parent.0);
            state
                .listed
                .entry(parent.0)
                .or_default()
                .insert(file.name.clone(), ino);
            ino
        };
        self.attr(file, ino)
    }

    /// Writes an open buffer back if its base version is still current; a sync that landed
    /// since it was read turns the buffer into a conflicted copy, which the handle then follows.
    fn commit(&self, fh: u64) -> Result<(), Errno> {
        let (id, base, buf) = {
            let state = self.state();
            let Some(h) = state.handles.get(&fh) else { return Ok(()) };
            if !h.dirty {
                return Ok(());
            }
            info!(op = "commit", fh, len = h.buf.len(), base = ?h.base.map(|b| b[0]));
            (h.id, h.base, h.buf.clone())
        };
        let len = buf.len() as u64;
        let (id, hmac) = match self.lb.safe_write(id, base, buf.clone(), None) {
            Ok(hmac) => (id, hmac),
            Err(err) if err.kind == LbErrKind::ReReadRequired => {
                let file = self.lb.get_file_by_id(id).map_err(errno)?;
                let name = conflict_name(&file.name);
                warn!("{} changed while open; saving these edits as {name}", file.name);
                let copy = self
                    .lb
                    .create_file(&name, &file.parent, FileType::Document)
                    .map_err(errno)?;
                (
                    copy.id,
                    self.lb
                        .safe_write(copy.id, None, buf, None)
                        .map_err(errno)?,
                )
            }
            Err(err) => return Err(errno(err)),
        };
        let mut state = self.state();
        state.sizes.insert(id, (Some(hmac), len));
        let pid = state.handles.get(&fh).map(|h| h.pid);
        if let Some(pid) = pid {
            state.served.insert((pid, id), Some(hmac));
        }
        if let Some(h) = state.handles.get_mut(&fh) {
            h.id = id;
            h.base = Some(hmac);
            h.dirty = false;
        }
        Ok(())
    }

    fn open_handle(
        &self, id: Uuid, truncate: bool, writable: bool, pid: u32,
    ) -> Result<u64, Errno> {
        let (current, mut buf) = self.lb.read_document_with_hmac(id, true).map_err(errno)?;
        let mut state = self.state();
        state.sizes.insert(id, (current, buf.len() as u64));
        // an in-place save writes what the process read earlier, not what this open sees
        let base = match state.served.get(&(pid, id)) {
            Some(read) if writable => *read,
            _ => current,
        };
        state.served.insert((pid, id), base);
        let dirty = truncate && !buf.is_empty();
        if truncate {
            buf.clear();
        }
        state.next_fh += 1;
        let fh = state.next_fh;
        state
            .handles
            .insert(fh, Handle { id, base, buf, dirty, pid, writable });
        Ok(fh)
    }

    /// Replaces `to`'s content with `from`'s and deletes `from`, keeping `to`'s id so shares and
    /// history follow the file. This is how most editors save (write a temp file, rename over).
    fn overwrite(&self, from: &File, to: &File, from_ino: u64, pid: u32) -> Result<(), Errno> {
        let content = self.lb.read_document(from.id, false).map_err(errno)?;
        let len = content.len() as u64;
        let served = self.state().served.get(&(pid, to.id)).copied();
        let base = match served {
            Some(hmac) => hmac,
            None => self.lb.document_hmac(to.id).map_err(errno)?,
        };
        match self.lb.safe_write(to.id, base, content, None) {
            Ok(hmac) => {
                self.lb.delete_file(&from.id).map_err(errno)?;
                let mut state = self.state();
                state.sizes.insert(to.id, (Some(hmac), len));
                state.served.insert((pid, to.id), Some(hmac));
                // the kernel now maps the new name to from's inode
                if let Some(old) = state.inos.insert(to.id, from_ino) {
                    state.ids.remove(&old);
                }
                state.ids.insert(from_ino, to.id);
                state.inos.remove(&from.id);
                Ok(())
            }
            Err(err) if err.kind == LbErrKind::ReReadRequired => {
                let name = conflict_name(&to.name);
                warn!("{} changed since it was read; saving this version as {name}", to.name);
                if from.parent != to.parent {
                    self.lb.move_file(&from.id, &to.parent).map_err(errno)?;
                }
                self.lb.rename_file(&from.id, &name).map_err(errno)
            }
            Err(err) => Err(errno(err)),
        }
    }
}

impl Filesystem for LbFuse {
    fn init(&mut self, _req: &Request, config: &mut KernelConfig) -> std::io::Result<()> {
        // O_TRUNC comes with open instead of as a separate truncation
        let _ = config.add_capabilities(InitFlags::FUSE_ATOMIC_O_TRUNC);
        Ok(())
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        self.touch();
        let result = self
            .id(parent)
            .and_then(|pid| self.child(pid, name))
            .and_then(|file| self.entry(parent, &file));
        match result {
            Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
            Err(e) => reply.error(e),
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        let result = self
            .id(ino)
            .and_then(|id| self.lb.get_file_by_id(id).map_err(errno))
            .and_then(|file| self.attr(&file, ino.0));
        match result {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(e) => reply.error(e),
        }
    }

    fn setattr(
        &self, _req: &Request, ino: INodeNo, _mode: Option<u32>, _uid: Option<u32>,
        _gid: Option<u32>, size: Option<u64>, _atime: Option<TimeOrNow>, _mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>, fh: Option<FileHandle>, _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>, _bkuptime: Option<SystemTime>,
        _flags: Option<fuser::BsdFileFlags>, reply: ReplyAttr,
    ) {
        info!(op = "setattr", ino = ino.0, ?size, fh = ?fh.map(|f| f.0));
        let result = (|| {
            let id = self.id(ino)?;
            if let Some(size) = size {
                // ftruncate arrives without a handle, so it lands in this file's open writers
                let mut state = self.state();
                let mut writers = state
                    .handles
                    .iter_mut()
                    .filter(|(k, h)| h.id == id && h.writable && fh.is_none_or(|fh| fh.0 == **k))
                    .map(|(_, h)| h)
                    .peekable();
                match writers.peek() {
                    Some(_) => {
                        for h in writers {
                            h.buf.resize(size as usize, 0);
                            h.dirty = true;
                        }
                    }
                    None => {
                        drop(state);
                        let (base, mut doc) =
                            self.lb.read_document_with_hmac(id, false).map_err(errno)?;
                        if doc.len() as u64 != size {
                            doc.resize(size as usize, 0);
                            self.lb.safe_write(id, base, doc, None).map_err(errno)?;
                        }
                    }
                }
            }
            let file = self.lb.get_file_by_id(id).map_err(errno)?;
            self.attr(&file, ino.0)
        })();
        match result {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(e) => reply.error(e),
        }
    }

    fn open(&self, req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        self.touch();
        let truncate = flags.0 & libc::O_TRUNC != 0;
        let writable = flags.acc_mode() != OpenAccMode::O_RDONLY;
        info!(op = "open", ino = ino.0, flags = format!("{:o}", flags.0), pid = process(req));
        match self
            .id(ino)
            .and_then(|id| self.open_handle(id, truncate, writable, process(req)))
        {
            // no FOPEN_KEEP_CACHE: every open rereads, so synced content shows on reopen
            Ok(fh) => reply.opened(FileHandle(fh), FopenFlags::empty()),
            Err(e) => reply.error(e),
        }
    }

    fn read(
        &self, _req: &Request, _ino: INodeNo, fh: FileHandle, offset: u64, size: u32,
        _flags: OpenFlags, _lock_owner: Option<LockOwner>, reply: ReplyData,
    ) {
        let state = self.state();
        let Some(h) = state.handles.get(&fh.0) else { return reply.error(Errno::EBADF) };
        let start = (offset as usize).min(h.buf.len());
        let end = (start + size as usize).min(h.buf.len());
        reply.data(&h.buf[start..end]);
    }

    fn write(
        &self, _req: &Request, _ino: INodeNo, fh: FileHandle, offset: u64, data: &[u8],
        _write_flags: WriteFlags, _flags: OpenFlags, _lock_owner: Option<LockOwner>,
        reply: ReplyWrite,
    ) {
        info!(op = "write", fh = fh.0, offset, len = data.len());
        let mut state = self.state();
        let Some(h) = state.handles.get_mut(&fh.0) else { return reply.error(Errno::EBADF) };
        let end = offset as usize + data.len();
        if h.buf.len() < end {
            h.buf.resize(end, 0);
        }
        h.buf[offset as usize..end].copy_from_slice(data);
        h.dirty = true;
        reply.written(data.len() as u32);
    }

    fn flush(
        &self, _req: &Request, _ino: INodeNo, fh: FileHandle, _lock_owner: LockOwner,
        reply: ReplyEmpty,
    ) {
        match self.commit(fh.0) {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn fsync(
        &self, _req: &Request, _ino: INodeNo, fh: FileHandle, _datasync: bool, reply: ReplyEmpty,
    ) {
        match self.commit(fh.0) {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn release(
        &self, _req: &Request, _ino: INodeNo, fh: FileHandle, _flags: OpenFlags,
        _lock_owner: Option<LockOwner>, _flush: bool, reply: ReplyEmpty,
    ) {
        info!(op = "release", fh = fh.0);
        let result = self.commit(fh.0);
        self.state().handles.remove(&fh.0);
        match result {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn readdir(
        &self, _req: &Request, ino: INodeNo, _fh: FileHandle, offset: u64,
        mut reply: ReplyDirectory,
    ) {
        self.touch();
        let result = (|| {
            let id = self.id(ino)?;
            let children = self.lb.get_children(&id).map_err(errno)?;
            let mut state = self.state();
            let parent = state.parents.get(&ino.0).copied().unwrap_or(ino.0);
            let mut entries = vec![
                (ino.0, Kind::Directory, ".".to_string()),
                (parent, Kind::Directory, "..".into()),
            ];
            let mut names = HashMap::new();
            for child in children {
                let child_ino = state.ino(child.id, ino.0);
                names.insert(child.name.clone(), child_ino);
                let kind = if child.is_folder() { Kind::Directory } else { Kind::RegularFile };
                entries.push((child_ino, kind, child.name));
            }
            state.listed.insert(ino.0, names);
            Ok::<_, Errno>(entries)
        })();
        match result {
            Ok(entries) => {
                for (i, (ino, kind, name)) in entries.into_iter().enumerate().skip(offset as usize)
                {
                    if reply.add(INodeNo(ino), (i + 1) as u64, kind, name) {
                        break;
                    }
                }
                reply.ok();
            }
            Err(e) => reply.error(e),
        }
    }

    fn create(
        &self, req: &Request, parent: INodeNo, name: &OsStr, _mode: u32, _umask: u32, _flags: i32,
        reply: ReplyCreate,
    ) {
        info!(op = "create", ?name, pid = process(req));
        let result = (|| {
            let pid = self.id(parent)?;
            let name = name.to_str().ok_or(Errno::EINVAL)?;
            let file = self
                .lb
                .create_file(name, &pid, FileType::Document)
                .map_err(errno)?;
            let attr = self.entry(parent, &file)?;
            let fh = self.open_handle(file.id, false, true, process(req))?;
            Ok::<_, Errno>((attr, fh))
        })();
        match result {
            Ok((attr, fh)) => {
                reply.created(&TTL, &attr, Generation(0), FileHandle(fh), FopenFlags::empty())
            }
            Err(e) => reply.error(e),
        }
    }

    fn mkdir(
        &self, _req: &Request, parent: INodeNo, name: &OsStr, _mode: u32, _umask: u32,
        reply: ReplyEntry,
    ) {
        let result = (|| {
            let pid = self.id(parent)?;
            let name = name.to_str().ok_or(Errno::EINVAL)?;
            let file = self
                .lb
                .create_file(name, &pid, FileType::Folder)
                .map_err(errno)?;
            self.entry(parent, &file)
        })();
        match result {
            Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
            Err(e) => reply.error(e),
        }
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let result = (|| {
            let file = self.child(self.id(parent)?, name)?;
            if file.is_folder() {
                return Err(Errno::EISDIR);
            }
            self.lb.delete_file(&file.id).map_err(errno)
        })();
        match result {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let result = (|| {
            let file = self.child(self.id(parent)?, name)?;
            if !file.is_folder() {
                return Err(Errno::ENOTDIR);
            }
            if !self.lb.get_children(&file.id).map_err(errno)?.is_empty() {
                return Err(Errno::ENOTEMPTY);
            }
            self.lb.delete_file(&file.id).map_err(errno)
        })();
        match result {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn rename(
        &self, req: &Request, parent: INodeNo, name: &OsStr, newparent: INodeNo, newname: &OsStr,
        flags: RenameFlags, reply: ReplyEmpty,
    ) {
        info!(op = "rename", ?name, ?newname, pid = process(req));
        let result = (|| {
            if flags.contains(RenameFlags::RENAME_EXCHANGE) {
                return Err(Errno::EINVAL);
            }
            let (pid, new_pid) = {
                let state = self.state();
                (state.id(parent)?, state.id(newparent)?)
            };
            let from = self.child(pid, name)?;
            let new_name = newname.to_str().ok_or(Errno::EINVAL)?;
            match self.child(new_pid, newname) {
                Ok(to) if to.id == from.id => Ok(()),
                Ok(_) if flags.contains(RenameFlags::RENAME_NOREPLACE) => Err(Errno::EEXIST),
                Ok(to) if from.is_document() && to.is_document() => {
                    let from_ino = self.state().ino(from.id, parent.0);
                    self.overwrite(&from, &to, from_ino, process(req))
                }
                Ok(to) if from.is_folder() && to.is_folder() => {
                    if !self.lb.get_children(&to.id).map_err(errno)?.is_empty() {
                        return Err(Errno::ENOTEMPTY);
                    }
                    self.lb.delete_file(&to.id).map_err(errno)?;
                    self.lb.move_file(&from.id, &new_pid).map_err(errno)?;
                    self.lb.rename_file(&from.id, new_name).map_err(errno)
                }
                Ok(to) if to.is_folder() => Err(Errno::EISDIR),
                Ok(_) => Err(Errno::ENOTDIR),
                Err(e) if e != Errno::ENOENT => Err(e),
                Err(_) => {
                    if pid != new_pid {
                        self.lb.move_file(&from.id, &new_pid).map_err(errno)?;
                    }
                    if from.name != new_name {
                        self.lb.rename_file(&from.id, new_name).map_err(errno)?;
                    }
                    let mut state = self.state();
                    let ino = state.ino(from.id, newparent.0);
                    state.parents.insert(ino, newparent.0);
                    Ok(())
                }
            }
        })();
        match result {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        // File managers refuse copies onto a volume that reports no free space.
        let blocks = 1 << 28;
        reply.statfs(blocks, blocks, blocks, 1 << 20, 1 << 20, 4096, 255, 4096);
    }
}

/// Watches lb-rs for changes from sync or other processes and drops what the kernel cached.
fn invalidate_on_change(lb: Lb, state: Arc<Mutex<State>>, notifier: Notifier) {
    let mut events = lb.subscribe();
    loop {
        let relevant = |e: &Event| {
            matches!(
                e,
                Event::MetadataChanged(Actor::Sync)
                    | Event::DocumentWritten(_, Actor::Sync)
                    | Event::IpcChangesApplied
            )
        };
        let mut changed = match events.blocking_recv() {
            Ok(e) => relevant(&e),
            Err(RecvError::Lagged(_)) => true,
            Err(RecvError::Closed) => return,
        };
        loop {
            match events.try_recv() {
                Ok(e) => changed |= relevant(&e),
                Err(TryRecvError::Lagged(_)) => changed = true,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => return,
            }
        }
        if changed {
            refresh(&lb, &state, &notifier);
        }
    }
}

fn refresh(lb: &Lb, state: &Mutex<State>, notifier: &Notifier) {
    let (listed, sizes, ids) = {
        let s = state.lock().unwrap();
        (s.listed.clone(), s.sizes.clone(), s.ids.clone())
    };
    let mut stale_entries = vec![];
    let mut stale_inodes = vec![];
    let mut relisted = HashMap::new();
    for (dir, old) in listed {
        let Some(id) = ids.get(&dir) else { continue };
        let Ok(children) = lb.get_children(id) else { continue };
        let now: HashMap<String, Uuid> = children.into_iter().map(|f| (f.name, f.id)).collect();
        let mut changed = now.len() != old.len();
        for (name, ino) in &old {
            if now
                .get(name)
                .map(|id| ids.get(ino) != Some(id))
                .unwrap_or(true)
            {
                stale_entries.push((dir, name.clone()));
                changed = true;
            }
        }
        if changed {
            stale_inodes.push(dir);
            relisted.insert(
                dir,
                old.into_iter()
                    .filter(|(n, _)| now.contains_key(n))
                    .collect(),
            );
        }
    }
    let mut s = state.lock().unwrap();
    for (id, (hmac, _)) in sizes {
        if lb.document_hmac(id).ok() != Some(hmac) {
            s.sizes.remove(&id);
            if let Some(ino) = s.inos.get(&id) {
                stale_inodes.push(*ino);
            }
        }
    }
    for (dir, names) in relisted {
        s.listed.insert(dir, names);
    }
    drop(s);
    info!("sync: {} names and {} inodes stale", stale_entries.len(), stale_inodes.len());
    for (dir, name) in stale_entries {
        let _ = notifier.inval_entry(INodeNo(dir), OsStr::new(&name));
    }
    for ino in stale_inodes {
        let _ = notifier.inval_inode(INodeNo(ino), 0, 0);
    }
}

/// Mounts until SIGINT or SIGTERM, then unmounts.
pub fn mount(lb: Lb, mountpoint: &Path) -> std::io::Result<()> {
    // a previous run that died leaves "Transport endpoint is not connected" behind
    if std::fs::metadata(mountpoint).is_err() {
        let _ = std::process::Command::new("fusermount3")
            .arg("-uz")
            .arg(mountpoint)
            .status();
    }
    std::fs::create_dir_all(mountpoint)?;

    let fs = LbFuse::new(lb.clone()).map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    let state = fs.state.clone();
    let mut config = FuseConfig::default();
    config.mount_options = vec![
        MountOption::FSName("lockbook".into()),
        MountOption::Subtype("lockbook".into()),
        MountOption::NoAtime,
        MountOption::NoDev,
        MountOption::NoSuid,
    ];
    let mut session = Session::new(fs, mountpoint, &config)?;
    let notifier = session.notifier();
    let mut unmounter = session.unmount_callable();
    std::thread::spawn(move || invalidate_on_change(lb, state, notifier));
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            use tokio::signal::unix::{SignalKind, signal};
            let mut term = signal(SignalKind::terminate()).unwrap();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
        });
        info!("unmounting");
        let _ = unmounter.unmount();
    });
    info!("mounted at {}", mountpoint.display());
    session.run()
}

pub fn default_mountpoint() -> PathBuf {
    std::env::home_dir().unwrap_or_default().join("Lockbook")
}
