use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use lb_rs::blocking::Lb;
use lb_rs::model::core_config::{ClientType, Config};
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_metadata::{DocumentHmac, FileType};
use lb_rs::model::filename::DocumentType;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::{LbErrKind, Uuid};
use windows::Win32::Storage::CloudFilters::*;

use crate::{cf, watch};

type Res<T> = Result<T, String>;

macro_rules! log {
    ($($t:tt)*) => {{
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        eprintln!("[{}.{:03}] {}", t.as_secs() % 100_000, t.subsec_millis(), format!($($t)*));
    }};
}

fn e<E: std::fmt::Debug>(err: E) -> String {
    format!("{err:?}")
}

const SYNC_EVERY: Duration = Duration::from_secs(3);
const SETTLE: Duration = Duration::from_millis(400);

/// What a placeholder on disk stands for.
#[derive(Clone)]
struct Known {
    path: PathBuf,
    folder: bool,
    /// Version the placeholder's metadata describes.
    hmac: Option<DocumentHmac>,
    /// Bytes an app last read or we last wrote: the base for merging the app's next save.
    base: Vec<u8>,
    base_hmac: Option<DocumentHmac>,
}

struct Entry {
    path: PathBuf,
    folder: bool,
    hmac: Option<DocumentHmac>,
}

struct Provider {
    lb: Lb,
    root: Uuid,
    dir: PathBuf,
    known: Mutex<HashMap<Uuid, Known>>,
    /// Ids whose disk delete or rename we're doing ourselves; their callbacks only ack.
    ours: Mutex<HashSet<Uuid>>,
}

static PROVIDER: OnceLock<Provider> = OnceLock::new();

pub fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| {
        args.get(i)
            .cloned()
            .ok_or_else(|| "missing argument".to_string())
    };
    let config = Config {
        client_name: "cloud-files".into(),
        writeable_path: Config::writeable_path("egui"),
        logs: true,
        client_type: ClientType::Ui,
        ..Default::default()
    };
    let lb = Lb::init(config).map_err(e)?;
    match arg(0)?.as_str() {
        "create-account" => {
            let url = std::env::var("API_URL").unwrap_or("http://localhost:8000".into());
            let account = lb.create_account(&arg(1)?, &url, false).map_err(e)?;
            println!("created {}", account.username);
        }
        "write" => {
            let file = match lb.get_by_path(&arg(1)?) {
                Ok(file) => file,
                Err(_) => lb.create_at_path(&arg(1)?).map_err(e)?,
            };
            lb.write_document(file.id, arg(2)?.as_bytes()).map_err(e)?;
            lb.sync().map_err(e)?;
        }
        "cat" => {
            lb.sync().map_err(e)?;
            let file = lb.get_by_path(&arg(1)?).map_err(e)?;
            print!("{}", String::from_utf8_lossy(&lb.read_document(file.id, false).map_err(e)?));
        }
        "share" => {
            let file = lb.get_by_path(&arg(1)?).map_err(e)?;
            lb.share_file(file.id, &arg(2)?, ShareMode::Write)
                .map_err(e)?;
            lb.sync().map_err(e)?;
        }
        "accept" => {
            lb.sync().map_err(e)?;
            for share in lb.get_pending_shares().map_err(e)? {
                lb.create_link_at_path(&share.name, share.id).map_err(e)?;
                println!("accepted {}", share.name);
            }
            lb.sync().map_err(e)?;
        }
        "sync" => lb.sync().map_err(e)?,
        "unregister" => cf::unregister(&lb.get_account().map_err(e)?.username).map_err(e)?,
        "run" => run(lb, PathBuf::from(arg(1)?))?,
        other => return Err(format!("unknown command {other}")),
    }
    Ok(())
}

fn run(lb: Lb, dir: PathBuf) -> Res<()> {
    if let Err(err) = lb.sync() {
        log!("initial sync failed: {err:?}");
    }
    let account = lb.get_account().map_err(e)?.username.clone();
    std::fs::create_dir_all(&dir).map_err(e)?;
    let dir = dir.canonicalize().map_err(e)?;
    // canonicalize adds \\?\, which cldapi paths never carry
    let dir = PathBuf::from(dir.to_string_lossy().trim_start_matches(r"\\?\"));
    if let Err(err) = cf::register(&dir, &account) {
        log!("register failed: {err:?}");
    }
    let root = lb.get_root().map_err(e)?.id;
    let p = PROVIDER.get_or_init(|| Provider {
        lb,
        root,
        dir: dir.clone(),
        known: Default::default(),
        ours: Default::default(),
    });
    let _key = cf::connect(&dir, on_fetch, on_delete, on_rename).map_err(e)?;
    log!("connected {} as {account}", dir.display());
    p.apply_remote()?;
    let (tx, rx) = mpsc::channel();
    watch::spawn(dir.clone(), tx);
    p.reconcile(&dir);

    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut last_event = Instant::now();
    let mut last_sync = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(path) => {
                pending.insert(path);
                last_event = Instant::now();
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err("watcher died".into()),
        }
        if !pending.is_empty() && last_event.elapsed() > SETTLE {
            let mut paths: Vec<_> = pending.drain().collect();
            paths.sort_by_key(|p| p.components().count());
            for path in paths {
                p.reconcile(&path);
            }
            last_sync = Instant::now() - SYNC_EVERY;
        }
        if pending.is_empty() && last_sync.elapsed() >= SYNC_EVERY {
            p.lb.refresh();
            if let Err(err) = p.lb.sync() {
                log!("sync failed: {err:?}");
            }
            if let Err(err) = p.apply_remote() {
                log!("apply failed: {err}");
            }
            last_sync = Instant::now();
        }
    }
}

fn windows_name_ok(name: &str) -> bool {
    !name.is_empty()
        && !name.ends_with(['.', ' '])
        && !name.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*'])
}

fn ignored(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with("~$")
        || lower.ends_with(".tmp")
        || lower == "desktop.ini"
        || lower == "thumbs.db"
}

fn merge(name: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> Option<Vec<u8>> {
    match DocumentType::from_file_name_using_extension(name) {
        DocumentType::Text => {
            let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
            Some(
                Buffer::from(text(base).as_str())
                    .merge(text(ours), text(theirs))
                    .into_bytes(),
            )
        }
        _ => None,
    }
}

impl Provider {
    fn rel(&self, abs: &Path) -> Option<PathBuf> {
        let abs = abs.to_string_lossy();
        let dir = self.dir.to_string_lossy();
        if abs.len() < dir.len() || !abs[..dir.len()].eq_ignore_ascii_case(&dir) {
            return None;
        }
        Some(PathBuf::from(abs[dir.len()..].trim_start_matches('\\')))
    }

    /// Lockbook's view of the tree, keyed by id, with paths relative to the sync root.
    fn tree(&self) -> Res<HashMap<Uuid, Entry>> {
        let files = self
            .lb
            .get_and_get_children_recursively(&self.root)
            .map_err(e)?;
        let by_id: HashMap<Uuid, _> = files.iter().map(|f| (f.id, f)).collect();
        let mut out = HashMap::new();
        'files: for f in &files {
            if f.id == self.root {
                continue;
            }
            let mut parts = vec![];
            let mut cur = f;
            loop {
                if !windows_name_ok(&cur.name) {
                    continue 'files;
                }
                parts.push(cur.name.clone());
                if cur.parent == self.root {
                    break;
                }
                match by_id.get(&cur.parent) {
                    Some(parent) => cur = *parent,
                    None => continue 'files,
                }
            }
            let folder = f.file_type == FileType::Folder;
            let hmac = if folder { None } else { self.lb.document_hmac(f.id).map_err(e)? };
            out.insert(f.id, Entry { path: parts.iter().rev().collect(), folder, hmac });
        }
        Ok(out)
    }

    fn moved(&self, old: &Path, new: &Path) {
        for k in self.known.lock().unwrap().values_mut() {
            if let Ok(rest) = k.path.strip_prefix(old) {
                k.path = new.join(rest);
            }
        }
    }

    /// Brings disk in line with Lockbook: new placeholders, moves, deletes, new content.
    fn apply_remote(&self) -> Res<()> {
        let tree = self.tree()?;
        let snapshot: HashMap<Uuid, Known> = self.known.lock().unwrap().clone();

        let mut gone: Vec<(&Uuid, &Known)> = snapshot
            .iter()
            .filter(|(id, _)| !tree.contains_key(id))
            .collect();
        gone.sort_by_key(|(_, k)| k.path.components().count());
        for (id, k) in gone {
            let abs = self.dir.join(&k.path);
            if std::fs::symlink_metadata(&abs).is_ok() {
                let ids: Vec<Uuid> = snapshot
                    .iter()
                    .filter(|(_, d)| d.path.starts_with(&k.path))
                    .map(|(id, _)| *id)
                    .collect();
                self.ours.lock().unwrap().extend(&ids);
                let result = if k.folder {
                    std::fs::remove_dir_all(&abs)
                } else {
                    std::fs::remove_file(&abs)
                };
                for id in &ids {
                    self.ours.lock().unwrap().remove(id);
                }
                log!("remote delete {}: {result:?}", k.path.display());
            }
            self.known.lock().unwrap().remove(id);
        }

        let mut entries: Vec<_> = tree.iter().collect();
        entries.sort_by_key(|(_, entry)| entry.path.components().count());
        for (id, entry) in entries {
            let abs = self.dir.join(&entry.path);
            let known = self.known.lock().unwrap().get(id).cloned();
            match known {
                None => self.place(*id, entry, &abs)?,
                Some(k) => {
                    if k.path != entry.path {
                        self.ours.lock().unwrap().insert(*id);
                        let result = std::fs::rename(self.dir.join(&k.path), &abs);
                        self.ours.lock().unwrap().remove(id);
                        log!(
                            "remote move {} -> {}: {result:?}",
                            k.path.display(),
                            entry.path.display()
                        );
                        if result.is_ok() {
                            self.moved(&k.path, &entry.path);
                        }
                    }
                    if !entry.folder && k.hmac != entry.hmac {
                        if let Some((_, true)) = cf::placeholder(&abs) {
                            let (hmac, doc) =
                                self.lb.read_document_with_hmac(*id, false).map_err(e)?;
                            match cf::replace_content(&abs, doc.len() as u64) {
                                Ok(()) => {
                                    log!(
                                        "remote edit {} ({} bytes)",
                                        entry.path.display(),
                                        doc.len()
                                    );
                                    if let Some(k) = self.known.lock().unwrap().get_mut(id) {
                                        k.hmac = hmac;
                                    }
                                }
                                Err(err) => {
                                    log!("remote edit {} deferred: {err:?}", entry.path.display())
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// First sighting of a Lockbook file: create its placeholder or adopt one from a previous run.
    fn place(&self, id: Uuid, entry: &Entry, abs: &Path) -> Res<()> {
        let (hmac, doc) = if entry.folder {
            (None, vec![])
        } else {
            self.lb.read_document_with_hmac(id, false).map_err(e)?
        };
        let known = Known {
            path: entry.path.clone(),
            folder: entry.folder,
            hmac,
            base: doc.clone(),
            base_hmac: hmac,
        };
        if std::fs::symlink_metadata(abs).is_ok() {
            match cf::placeholder(abs) {
                Some((existing, in_sync)) if existing == id => {
                    if !entry.folder && in_sync {
                        let _ = cf::replace_content(abs, doc.len() as u64);
                    }
                    self.known.lock().unwrap().insert(id, known);
                }
                // A local item holds this name; reconcile matches it by name.
                _ => {}
            }
            return Ok(());
        }
        let parent = abs.parent().unwrap();
        let name = abs.file_name().unwrap().to_string_lossy();
        cf::create_placeholder(parent, &name, id, entry.folder, doc.len() as u64).map_err(e)?;
        log!("placeholder {}", entry.path.display());
        self.known.lock().unwrap().insert(id, known);
        Ok(())
    }

    /// Picks up local changes Windows doesn't report through callbacks: saves and new items.
    fn reconcile(&self, abs: &Path) {
        let Some(rel) = self.rel(abs) else { return };
        if rel.as_os_str().is_empty() {
            for child in std::fs::read_dir(abs).into_iter().flatten().flatten() {
                self.reconcile(&child.path());
            }
            return;
        }
        if rel
            .components()
            .any(|c| ignored(&c.as_os_str().to_string_lossy()))
        {
            return;
        }
        let Ok(meta) = std::fs::symlink_metadata(abs) else { return };
        let result = match cf::placeholder(abs) {
            Some((_, true)) => Ok(()),
            Some((_, _)) if meta.is_dir() => Ok(()),
            Some((id, false)) => self.push(id, abs, &rel, false),
            None => self.adopt(abs, &rel, meta.is_dir()),
        };
        if let Err(err) = result {
            log!("reconcile {} failed: {err}", rel.display());
        }
    }

    fn dir_id(&self, dir: &Path) -> Res<Uuid> {
        if self.rel(dir).is_some_and(|r| r.as_os_str().is_empty()) {
            return Ok(self.root);
        }
        if let Some((id, _)) = cf::placeholder(dir) {
            return Ok(id);
        }
        self.reconcile(dir);
        cf::placeholder(dir)
            .map(|(id, _)| id)
            .ok_or_else(|| format!("{} untracked", dir.display()))
    }

    /// A plain file or folder: new, or an app's save that replaced the placeholder.
    fn adopt(&self, abs: &Path, rel: &Path, folder: bool) -> Res<()> {
        let parent = self.dir_id(abs.parent().unwrap())?;
        let name = abs.file_name().unwrap().to_string_lossy().into_owned();
        let existing = self
            .lb
            .get_children(&parent)
            .map_err(e)?
            .into_iter()
            .find(|f| {
                f.name.eq_ignore_ascii_case(&name) && (f.file_type == FileType::Folder) == folder
            });
        let typ = if folder { FileType::Folder } else { FileType::Document };
        let id = match existing {
            Some(f) => f.id,
            None => {
                log!("local create {}", rel.display());
                self.lb.create_file(&name, &parent, typ).map_err(e)?.id
            }
        };
        if folder {
            cf::convert(abs, id).map_err(e)?;
            let known =
                Known { path: rel.into(), folder, hmac: None, base: vec![], base_hmac: None };
            self.known.lock().unwrap().insert(id, known);
            for child in std::fs::read_dir(abs).into_iter().flatten().flatten() {
                self.reconcile(&child.path());
            }
            Ok(())
        } else {
            self.push(id, abs, rel, true)
        }
    }

    /// Writes an app's save into Lockbook, three-way merging with anything newer.
    fn push(&self, id: Uuid, abs: &Path, rel: &Path, convert: bool) -> Res<()> {
        let mut ours = None;
        for _ in 0..20 {
            match std::fs::read(abs) {
                Ok(bytes) => {
                    ours = Some(bytes);
                    break;
                }
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
        let ours = ours.ok_or("file stayed locked")?;
        let name = abs.file_name().unwrap().to_string_lossy().into_owned();
        let known = self.known.lock().unwrap().get(&id).cloned();
        let (written, hmac) = loop {
            let (current, theirs) = self.lb.read_document_with_hmac(id, false).map_err(e)?;
            let (base, base_hmac) = match &known {
                Some(k) => (k.base.clone(), k.base_hmac),
                None => (theirs.clone(), current),
            };
            let out = if base_hmac == current || theirs == ours {
                ours.clone()
            } else if let Some(merged) = merge(&name, &base, &ours, &theirs) {
                log!("merged {} with a newer version", rel.display());
                merged
            } else {
                // Not mergeable: keep theirs here and ours beside it.
                let parent = self.lb.get_file_by_id(id).map_err(e)?.parent;
                let copy = format!("{name} (Windows copy)");
                let file = self
                    .lb
                    .create_file(&copy, &parent, FileType::Document)
                    .map_err(e)?;
                self.lb.write_document(file.id, &ours).map_err(e)?;
                log!("kept both {} and {copy}", rel.display());
                theirs.clone()
            };
            if out == theirs {
                break (out, current);
            }
            match self.lb.safe_write(id, current, out.clone(), None) {
                Ok(hmac) => break (out, Some(hmac)),
                Err(err) if matches!(err.kind, LbErrKind::ReReadRequired) => continue,
                Err(err) => return Err(e(err)),
            }
        };
        if written != ours {
            std::fs::write(abs, &written).map_err(e)?;
        }
        if convert {
            cf::convert(abs, id).map_err(e)?;
        } else {
            cf::mark_in_sync(abs).map_err(e)?;
        }
        log!("local edit {} ({} bytes)", rel.display(), written.len());
        self.known.lock().unwrap().insert(
            id,
            Known { path: rel.into(), folder: false, hmac, base: written, base_hmac: hmac },
        );
        Ok(())
    }

    fn fetch(&self, info: &CF_CALLBACK_INFO) -> Res<()> {
        let id = cf::identity(info).ok_or("no identity")?;
        let read = self.lb.read_document_with_hmac(id, false).map_err(e);
        let read = read.and_then(|(hmac, doc)| {
            if doc.len() as i64 == info.FileSize {
                Ok((hmac, doc))
            } else {
                Err(format!("size changed: {} != {}", doc.len(), info.FileSize))
            }
        });
        match read {
            Ok((hmac, doc)) => {
                cf::transfer(info, Ok(&doc)).map_err(e)?;
                log!("hydrated {id} ({} bytes)", doc.len());
                if let Some(k) = self.known.lock().unwrap().get_mut(&id) {
                    k.base = doc;
                    k.base_hmac = hmac;
                }
                Ok(())
            }
            Err(err) => {
                cf::transfer(info, Err(windows::core::Error::empty())).map_err(e)?;
                Err(err)
            }
        }
    }

    fn delete(&self, info: &CF_CALLBACK_INFO) -> Res<()> {
        let id = cf::identity(info).ok_or("no identity")?;
        if self.ours.lock().unwrap().contains(&id) || self.lb.get_file_by_id(id).is_err() {
            return cf::ack_delete(info, true).map_err(e);
        }
        let result = self.lb.delete_file(&id);
        cf::ack_delete(info, result.is_ok()).map_err(e)?;
        result.map_err(e)?;
        self.known.lock().unwrap().remove(&id);
        log!("local delete {id}");
        Ok(())
    }

    fn rename(&self, info: &CF_CALLBACK_INFO, params: &CF_CALLBACK_PARAMETERS) -> Res<()> {
        let id = cf::identity(info).ok_or("no identity")?;
        if self.ours.lock().unwrap().contains(&id) {
            return cf::ack_rename(info, true).map_err(e);
        }
        let rename = unsafe { params.Anonymous.Rename };
        let target = PathBuf::from(format!(
            "{}{}",
            cf::pcwstr(info.VolumeDosName),
            cf::pcwstr(rename.TargetPath)
        ));
        let result = (|| {
            if (rename.Flags.0 & CF_CALLBACK_RENAME_FLAG_TARGET_IN_SCOPE.0) == 0 {
                log!("moved out of Lockbook, deleting {id}");
                return self.lb.delete_file(&id).map_err(e);
            }
            let parent = self.dir_id(target.parent().unwrap())?;
            let name = target.file_name().unwrap().to_string_lossy().into_owned();
            let file = self.lb.get_file_by_id(id).map_err(e)?;
            if file.parent != parent {
                self.lb.move_file(&id, &parent).map_err(e)?;
            }
            if file.name != name {
                self.lb.rename_file(&id, &name).map_err(e)?;
            }
            Ok(())
        })();
        cf::ack_rename(info, result.is_ok()).map_err(e)?;
        result?;
        let old = self.known.lock().unwrap().get(&id).map(|k| k.path.clone());
        if let (Some(old), Some(new)) = (old, self.rel(&target)) {
            self.moved(&old, &new);
        }
        log!("local rename {id} -> {}", target.display());
        Ok(())
    }
}

fn provider() -> &'static Provider {
    PROVIDER.get().expect("callback before init")
}

unsafe extern "system" fn on_fetch(
    info: *const CF_CALLBACK_INFO, _: *const CF_CALLBACK_PARAMETERS,
) {
    if let Err(err) = provider().fetch(unsafe { &*info }) {
        log!("fetch failed: {err}");
    }
}

unsafe extern "system" fn on_delete(
    info: *const CF_CALLBACK_INFO, _: *const CF_CALLBACK_PARAMETERS,
) {
    if let Err(err) = provider().delete(unsafe { &*info }) {
        log!("delete failed: {err}");
    }
}

unsafe extern "system" fn on_rename(
    info: *const CF_CALLBACK_INFO, params: *const CF_CALLBACK_PARAMETERS,
) {
    if let Err(err) = provider().rename(unsafe { &*info }, unsafe { &*params }) {
        log!("rename failed: {err}");
    }
}
