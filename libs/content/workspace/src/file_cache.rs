use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::iter;

use db_rs::hasher::UuidIdentityHasherBuilder;
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::access_info::UserAccessMode;
use lb_rs::model::account::Account;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file::{File, ShareMode};
use lb_rs::model::file_metadata::FileType;
use tracing::instrument;
use urlencoding::decode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedLink {
    File(Uuid),
    External(String),
}

/// How a linked document relates to the scope of the note linking to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// Inside the note's scope: the same file for every participant.
    Scope,
    /// Past the scope: a different file, or none, depending on who reads.
    Reader,
}

type UuidMap<V> = HashMap<Uuid, V, UuidIdentityHasherBuilder>;

fn uuid_map<V>() -> UuidMap<V> {
    HashMap::with_hasher(UuidIdentityHasherBuilder)
}

fn uuid_map_with_capacity<V>(n: usize) -> UuidMap<V> {
    HashMap::with_capacity_and_hasher(n, UuidIdentityHasherBuilder)
}

pub struct FileCache {
    pub root: File,
    /// Clustered covering index: own tree + pending shares, sorted by
    /// `(parent, is_document, name)`. `(parent, name)` is unique.
    rows: Vec<File>,
    by_id: UuidMap<u32>,
    pub shared_roots: Vec<File>,
    pub suggested: Vec<Uuid>,
    pub size_bytes_recursive: UuidMap<u64>,
    pub last_modified_recursive: UuidMap<u64>,
    /// Max last_modified across all files. Used as a cache invalidation key
    /// by link completions — changes whenever the file tree changes.
    pub last_modified: u64,
}

/// Folders first, then name. Parent+name is unique so `id` is not in the key.
fn cmp_cluster(a: &File, b: &File) -> Ordering {
    a.parent
        .cmp(&b.parent)
        .then_with(|| a.is_document().cmp(&b.is_document()))
        .then_with(|| a.name.cmp(&b.name))
}

fn index_rows(rows: &[File]) -> UuidMap<u32> {
    rows.iter()
        .enumerate()
        .map(|(i, f)| (f.id, i as u32))
        .collect()
}

impl FileCache {
    /// An empty file cache for contexts where no real files exist (e.g. public site demos).
    pub fn empty() -> Self {
        let root_id = Uuid::new_v4();
        let root = File {
            id: root_id,
            parent: root_id,
            name: "root".into(),
            file_type: FileType::Folder,
            last_modified: 0,
            last_modified_by: String::new(),
            owner: String::new(),
            shares: vec![],
            size_bytes: 0,
        };
        Self {
            root: root.clone(),
            suggested: vec![],
            size_bytes_recursive: Default::default(),
            last_modified_recursive: Default::default(),
            last_modified: 0,
            shared_roots: vec![],
            rows: vec![root],
            by_id: [(root_id, 0)].into_iter().collect(),
        }
    }

    /// Own tree + pending-share rows as a single clustered covering index.
    pub fn from_owned_and_shared(
        root: File, owned: impl IntoIterator<Item = File>, shared: impl IntoIterator<Item = File>,
    ) -> Self {
        Self::from_rows(root, owned.into_iter().chain(shared), Vec::new(), Vec::new())
    }

    fn from_rows(
        root: File, files: impl IntoIterator<Item = File>, shared_roots: Vec<File>,
        suggested: Vec<Uuid>,
    ) -> Self {
        let mut rows: Vec<File> = files.into_iter().collect();
        if !rows.iter().any(|f| f.id == root.id) {
            rows.push(root.clone());
        }
        rows.sort_by(cmp_cluster);
        let last_modified = rows.iter().map(|f| f.last_modified).max().unwrap_or(0);
        let by_id = index_rows(&rows);
        Self {
            root,
            rows,
            by_id,
            shared_roots,
            suggested,
            size_bytes_recursive: uuid_map(),
            last_modified_recursive: uuid_map(),
            last_modified,
        }
    }

    #[instrument(name = "FileCache::new", level = "trace", skip_all, fields(n_files = tracing::field::Empty))]
    pub fn new(lb: &Lb) -> LbResult<Self> {
        let root = lb.get_root()?;
        let files = lb.list_metadatas()?;
        let suggested = lb.suggested_docs(Default::default())?;
        let shared = lb.get_pending_share_files()?;
        let shared_roots = lb.get_pending_shares()?;
        tracing::Span::current().record("n_files", files.len() + shared.len());
        let mut cache =
            Self::from_rows(root, files.into_iter().chain(shared), shared_roots, suggested);
        cache.fill_recursive();
        Ok(cache)
    }

    #[instrument(level = "trace", skip_all, fields(n))]
    fn fill_recursive(&mut self) {
        tracing::Span::current().record("n", self.rows.len());
        let ids: Vec<Uuid> = self.rows.iter().map(|f| f.id).collect();
        let mut size = uuid_map_with_capacity(ids.len());
        let mut modified = uuid_map_with_capacity(ids.len());
        for id in ids {
            let me = self.get_by_id(id).unwrap();
            let mut sum = me.size_bytes;
            let mut best_mod = me.last_modified;
            for f in self.descendents(id) {
                sum += f.size_bytes;
                best_mod = best_mod.max(f.last_modified);
            }
            size.insert(id, sum);
            modified.insert(id, best_mod);
        }
        self.size_bytes_recursive = size;
        self.last_modified_recursive = modified;
    }

    pub fn usage_portion(&self, id: Uuid) -> f32 {
        self.size_bytes_recursive[&id] as f32
            / self.size_bytes_recursive[&self.get_by_id(id).unwrap().parent] as f32
    }

    pub fn last_modified_recursive(&self, id: Uuid) -> u64 {
        self.last_modified_recursive
            .get(&id)
            .copied()
            .unwrap_or_else(|| self.get_by_id(id).map(|f| f.last_modified).unwrap_or(0))
    }

    /// Iterates all known files: the user's own tree plus pending shares.
    pub fn all_files(&self) -> impl Iterator<Item = &File> {
        self.rows.iter()
    }

    /// `(file, path)` pairs for the path searcher used by link completions.
    pub fn path_index(&self) -> Vec<(File, String)> {
        self.all_files()
            .filter(|f| !f.is_root())
            .map(|f| (f.clone(), self.path(f.id)))
            .collect()
    }

    pub fn insert_created_file(&mut self, file: File) {
        let file_id = file.id;
        let file_size = file.size_bytes;
        let file_modified = file.last_modified;

        let idx = self
            .rows
            .partition_point(|f| cmp_cluster(f, &file) == Ordering::Less);
        self.rows.insert(idx, file);
        for i in idx..self.rows.len() {
            self.by_id.insert(self.rows[i].id, i as u32);
        }

        self.size_bytes_recursive.insert(file_id, file_size);
        self.last_modified_recursive.insert(file_id, file_modified);
        self.last_modified = self.last_modified.max(file_modified);

        for ancestor in self.ancestors(file_id) {
            let ancestor_modified = self
                .last_modified_recursive
                .entry(ancestor)
                .or_insert(file_modified);
            *ancestor_modified = (*ancestor_modified).max(file_modified);
        }
    }
}

impl Debug for FileCache {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCache")
            .field("rows.len()", &self.rows.len())
            .field("suggested.len()", &self.suggested.len())
            .finish()
    }
}

pub trait FilesExt {
    fn root(&self) -> &File;
    fn get_by_id(&self, id: Uuid) -> Option<&File>;
    fn children(&self, id: Uuid) -> Vec<&File>;
    fn iter_files(&self) -> impl Iterator<Item = &File>;

    fn siblings(&self, id: Uuid) -> Vec<&File> {
        let parent = self.get_by_id(id).unwrap().parent;
        self.children(parent)
            .into_iter()
            .filter(|f| f.id != id)
            .collect()
    }

    fn descendents(&self, id: Uuid) -> Vec<&File> {
        let mut descendents = vec![];
        for child in self.children(id) {
            descendents.extend(self.descendents(child.id));
            descendents.push(child);
        }
        descendents
    }

    /// Walks ancestors to find the tree root: the user's own root or the topmost
    /// reachable file (a pending share root, whose parent is not in the cache).
    fn tree_root(&self, id: Uuid) -> Uuid {
        let mut current = id;
        loop {
            let Some(file) = self.get_by_id(current) else { return current };
            if file.is_root() {
                return current;
            }
            if self.get_by_id(file.parent).is_none() {
                return current;
            }
            current = file.parent;
        }
    }

    fn same_tree(&self, a: Uuid, b: Uuid) -> bool {
        self.tree_root(a) == self.tree_root(b)
    }

    /// Returns the path string for a file. Own-tree paths start with `/`;
    /// pending share-tree paths have no leading `/` (they have no absolute address).
    fn path(&self, id: Uuid) -> String {
        let Some(file) = self.get_by_id(id) else { return "/".to_string() };
        if file.is_root() {
            return "/".to_string();
        }
        let mut parts = vec![file.name.as_str()];
        let mut current = file.parent;
        let mut reached_root = false;
        while let Some(f) = self.get_by_id(current) {
            if f.is_root() {
                reached_root = true;
                break;
            }
            parts.push(f.name.as_str());
            current = f.parent;
        }
        parts.reverse();
        let joined = parts.join("/");
        if reached_root && file.is_folder() {
            format!("/{joined}/")
        } else if reached_root {
            format!("/{joined}")
        } else if file.is_folder() {
            format!("{joined}/")
        } else {
            joined
        }
    }

    fn by_path(&self, path: &str) -> Option<&File> {
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = self.root().id;
        for component in components {
            current = self
                .children(current)
                .into_iter()
                .find(|f| f.name == component)?
                .id;
        }
        self.get_by_id(current)
    }

    /// Top of a note's scope: its innermost shared ancestor, itself included,
    /// else the top of its tree. Every participant holds everything under it,
    /// wherever they placed their share, so links resolved inside it mean the
    /// same file to all of them.
    fn scope_top(&self, note: Uuid) -> Uuid {
        let mut current = note;
        loop {
            let Some(file) = self.get_by_id(current) else { return current };
            if !file.shares.is_empty() || file.is_root() || self.get_by_id(file.parent).is_none() {
                return current;
            }
            current = file.parent;
        }
    }

    /// Whether `id` is `scope` or lies under it.
    fn in_scope(&self, scope: Uuid, id: Uuid) -> bool {
        id == scope || self.ancestors(id).contains(&scope)
    }

    /// Walks `rel` from the folder `from` without leaving `scope`: `..` at
    /// the scope's top fails, as does starting outside it.
    fn walk_in_scope(&self, scope: Uuid, from: Uuid, rel: &str) -> Option<&File> {
        if !self.in_scope(scope, from) {
            return None;
        }
        let mut current = from;
        for component in rel.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if current == scope {
                        return None;
                    }
                    current = self.get_by_id(current)?.parent;
                }
                name => {
                    current = self
                        .children(current)
                        .into_iter()
                        .find(|f| f.name == name)?
                        .id;
                }
            }
        }
        self.get_by_id(current)
    }

    /// The document a link or image in `note` reaches, and how.
    ///
    /// - id link (`lb://uuid`, or the external URL `…/open/uuid`) — any
    ///   document here; [`Reach::Reader`] outside the note's scope.
    /// - absolute path (`/foo`) — the reader's own root; [`Reach::Reader`]
    ///   when the note is shared.
    /// - relative path — walked from the note's folder inside the note's
    ///   scope ([`Self::scope_top`]); failing that, anywhere in the reader's
    ///   tree as [`Reach::Reader`].
    ///
    /// Only documents resolve; folders are treated as broken. None for
    /// anything else, web URLs included.
    fn locate_link(&self, url: &str, note: Uuid) -> Option<(Uuid, Reach)> {
        let scope = self.scope_top(note);
        self.locate_link_in(url, note, scope).or_else(|| {
            let extent = self.tree_root(note);
            if extent == scope {
                return None;
            }
            let (id, _) = self.locate_link_in(url, note, extent)?;
            Some((id, Reach::Reader))
        })
    }

    /// [`Self::locate_link`] with `scope` standing in for the note's own.
    fn locate_link_in(&self, url: &str, note: Uuid, scope: Uuid) -> Option<(Uuid, Reach)> {
        let source = self.get_by_id(note)?;
        if let Some(id) = link_id(url) {
            let file = self.get_by_id(id).filter(|f| f.is_document())?;
            // a note under no share has only one reader
            let unshared = scope == self.root().id;
            let reach =
                if unshared || self.in_scope(scope, id) { Reach::Scope } else { Reach::Reader };
            return Some((file.id, reach));
        }
        if url.starts_with('#') {
            return None;
        }

        let (file, reach) = if url.starts_with('/') {
            let canonical = canonicalize(url);
            let decoded = decode(&canonical)
                .map(|c| c.into_owned())
                .unwrap_or(canonical);
            let reach = if scope == self.root().id { Reach::Scope } else { Reach::Reader };
            (self.by_path(&decoded)?, reach)
        } else {
            let decoded = decode(url)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| url.to_string());
            (self.walk_in_scope(scope, source.parent, &decoded)?, Reach::Scope)
        };
        file.is_document().then_some((file.id, reach))
    }

    /// Resolves the destination of a link in `note`: a document by
    /// [`Self::locate_link`], whatever its reach, or a web URL. Returns None
    /// for an internal destination that doesn't resolve.
    fn resolve_link(&self, url: &str, note: Uuid) -> Option<ResolvedLink> {
        match self.locate_link(url, note) {
            Some((id, _)) => Some(ResolvedLink::File(id)),
            None => web_link(url).then(|| ResolvedLink::External(url.to_string())),
        }
    }

    /// Resolves a wikilink title in `note`: a document in its scope, else one
    /// past it ([`Self::locate_wikilink`]).
    ///
    /// As in Obsidian, `.md` is optional in the link and other extensions are
    /// not: `note` matches `note.md` or `note`, and a drawing is `note.svg`.
    /// An exact full-name match wins over one without `.md`.
    ///
    /// - path titles (`folder/note`) resolve the folder relative to the
    ///   note's folder, then match the final component among its documents.
    /// - bare titles match below the scope's top; the nearest match wins.
    ///
    /// Only documents match; folders are ignored. Returns None when nothing
    /// matches or the match is ambiguous (multiple equally-specific,
    /// equally-near documents) — adding an extension or a path disambiguates.
    fn resolve_wikilink(&self, title: &str, note: Uuid) -> Option<Uuid> {
        self.locate_wikilink(title, note).map(|(id, _)| id)
    }

    /// [`Self::resolve_wikilink`], and how it reaches: a title with no match
    /// at all in the scope may match anywhere in the reader's tree, as
    /// [`Reach::Reader`].
    fn locate_wikilink(&self, title: &str, note: Uuid) -> Option<(Uuid, Reach)> {
        let scope = self.scope_top(note);
        if let Some(id) = self.resolve_wikilink_in(title, note, scope) {
            return Some((id, Reach::Scope));
        }
        let in_scope = |f: &File| f.id != scope && self.in_scope(scope, f.id);
        let extent = self.tree_root(note);
        if extent == scope
            || (!title.contains('/')
                && self
                    .iter_files()
                    .any(|f| f.is_document() && title_matches(&f.name, title) && in_scope(f)))
        {
            return None;
        }
        // the scope's top answers to no title, there or past it
        let id = self.resolve_wikilink_in(title, note, extent)?;
        (id != scope).then_some((id, Reach::Reader))
    }

    /// [`Self::resolve_wikilink`] with `scope` standing in for the note's own.
    fn resolve_wikilink_in(&self, title: &str, note: Uuid, scope: Uuid) -> Option<Uuid> {
        match self.wikilink_nearest_in(title, note, scope)[..] {
            [id] => Some(id),
            _ => None,
        }
    }

    /// The documents `title` names equally well in `scope`: one when it
    /// resolves, several when it is ambiguous.
    ///
    /// A title with folders in it (`design/Plan`) is a path from the note's
    /// folder, else names a document whose folders end with it, anywhere in
    /// the scope. One that leads with `/` is a path from the scope's top.
    /// The spaces around a segment don't count.
    fn wikilink_nearest_in(&self, title: &str, note: Uuid, scope: Uuid) -> Vec<Uuid> {
        let Some(from) = self.get_by_id(note).map(|f| f.parent) else { return vec![] };
        let (dir, last) = title.rsplit_once('/').unwrap_or(("", title));
        let last = last.trim();
        let dirs: Vec<&str> = dir
            .split('/')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        // the top's name is the placer's choice, so only files under it match
        let in_scope = |f: &&File| f.is_document() && f.id != scope && self.in_scope(scope, f.id);
        // names first: the scope check walks ancestors
        let named = |f: &&File| title_matches(&f.name, last) && in_scope(f);
        // `dirs` walked from `start` without leaving the scope, case aside
        let walked = |start: Uuid| {
            let mut current = start;
            for dir in &dirs {
                current = match *dir {
                    "." => current,
                    ".." if current == scope => return vec![],
                    ".." => match self.get_by_id(current) {
                        Some(folder) => folder.parent,
                        None => return vec![],
                    },
                    name => {
                        let folder = |f: &&File| f.is_folder() && f.name.eq_ignore_ascii_case(name);
                        match self.children(current).into_iter().find(folder) {
                            Some(folder) => folder.id,
                            None => return vec![],
                        }
                    }
                };
            }
            self.children(current).into_iter().filter(named).collect()
        };
        let trailing = || {
            self.iter_files()
                .filter(named)
                .filter(|f| self.trails(scope, f, &dirs))
                .collect()
        };
        let candidates: Vec<&File> = if title.starts_with('/') {
            walked(scope)
        } else if dirs.iter().any(|s| matches!(*s, "." | "..")) {
            walked(from)
        } else if dirs.is_empty() {
            trailing()
        } else {
            match walked(from) {
                found if !found.is_empty() => found,
                _ => trailing(),
            }
        };

        // Exact full-name matches outrank stem-only matches.
        let exact: Vec<&File> = candidates
            .iter()
            .copied()
            .filter(|f| f.name.eq_ignore_ascii_case(last))
            .collect();
        let pool = if exact.is_empty() { candidates } else { exact };

        // Nearest wins; a tie at the minimum distance is ambiguous.
        let from_path = self.path(from);
        let distance = |f: &File| {
            relative_path(&from_path, &self.path(f.id))
                .matches('/')
                .count()
        };
        let Some(nearest) = pool.iter().map(|f| distance(f)).min() else { return vec![] };
        pool.into_iter()
            .filter(|f| distance(f) == nearest)
            .map(|f| f.id)
            .collect()
    }

    /// Whether the folders above `file`, strictly inside `scope`, end with
    /// `dirs`.
    fn trails(&self, scope: Uuid, file: &File, dirs: &[&str]) -> bool {
        let mut current = file;
        for dir in dirs.iter().rev() {
            if current.id == scope {
                return false;
            }
            let Some(folder) = self.get_by_id(current.parent) else { return false };
            if folder.id == scope || !folder.name.eq_ignore_ascii_case(dir) {
                return false;
            }
            current = folder;
        }
        true
    }

    /// Folders between a document and the folder a link is written in: up
    /// from `from` to their common ancestor, then down to `id`'s folder.
    fn folder_distance(&self, from: Uuid, id: Uuid) -> usize {
        let chain = |start: Uuid| {
            let mut chain = vec![start];
            chain.extend(self.ancestors(start));
            chain
        };
        let from_chain = chain(from);
        let to_chain = chain(self.get_by_id(id).map(|f| f.parent).unwrap_or(id));
        for (down, folder) in to_chain.iter().enumerate() {
            if let Some(up) = from_chain.iter().position(|f| f == folder) {
                return up + down;
            }
        }
        from_chain.len() + to_chain.len()
    }

    /// The destination a link in `note` to `target` is written with: a
    /// relative path inside the note's scope. None outside it, where only an
    /// id link reaches.
    fn link_destination(&self, target: Uuid, note: Uuid) -> Option<String> {
        let scope = self.scope_top(note);
        let from = self.get_by_id(note).map(|f| f.parent).unwrap_or(note);
        if !self.in_scope(scope, from) || !self.in_scope(scope, target) {
            return None;
        }
        let path = encode_link_path(&relative_path(&self.path(from), &self.path(target)));
        // `Re: plan.md` would read as a URL
        Some(if has_scheme(&path) { format!("./{path}") } else { path })
    }

    /// The shortest wikilink title that means `target` anywhere in `note`'s
    /// scope: its title, else with enough folders in front, else its full
    /// name likewise. A document whose every trailing path also ends a
    /// deeper one's gets its whole path from the scope's top. None outside
    /// the scope.
    fn wikilink_title(&self, target: Uuid, note: Uuid) -> Option<String> {
        self.wikilink_title_in(target, self.scope_top(note))
    }

    /// [`Self::wikilink_title`] with `scope` standing in for the note's own:
    /// the reader's whole tree ([`Self::tree_root`]) for a title that will
    /// resolve past the scope.
    fn wikilink_title_in(&self, target: Uuid, scope: Uuid) -> Option<String> {
        let file = self.get_by_id(target)?;
        if target == scope || !self.in_scope(scope, target) {
            return None;
        }
        // folders above the document, nearest first, up to the scope's top
        let mut dirs: Vec<&str> = vec![];
        let mut current = file;
        while let Some(parent) = self.get_by_id(current.parent) {
            if parent.id == scope {
                break;
            }
            dirs.push(&parent.name);
            current = parent;
        }
        let unique = |last: &str, dirs: &[&str]| {
            let mut path: Vec<&str> = dirs.iter().rev().copied().collect();
            let matches = self
                .iter_files()
                .filter(|f| f.is_document() && title_matches(&f.name, last))
                .filter(|f| {
                    f.id != scope && self.in_scope(scope, f.id) && self.trails(scope, f, &path)
                })
                .count();
            path.push(last);
            (matches == 1).then(|| escape_wiki_title(&path.join("/")))
        };
        let (title, name) = (wiki_title(&file.name), file.name.as_str());
        [title, name]
            .into_iter()
            .flat_map(|last| (0..=dirs.len()).map(move |n| (last, n)))
            .find_map(|(last, n)| unique(last, &dirs[..n]))
            .or_else(|| {
                let beside = self.children(file.parent);
                let titles = beside
                    .iter()
                    .filter(|f| f.is_document() && title_matches(&f.name, title));
                let last = if titles.count() == 1 { title } else { name };
                let dirs = dirs.iter().rev().map(|d| format!("{d}/"));
                Some(escape_wiki_title(&format!("/{}{last}", dirs.collect::<String>())))
            })
    }

    fn ancestors(&self, id: Uuid) -> Vec<Uuid> {
        let mut ancestors = vec![];
        let mut current = id;
        while let Some(file) = self.get_by_id(current) {
            if file.is_root() {
                break;
            }
            let parent = file.parent;
            if self.get_by_id(parent).is_none() {
                break; // share boundary: parent not in cache
            }
            ancestors.push(parent);
            current = parent;
        }
        ancestors
    }

    fn access(&self, id: Uuid, account: &Account) -> UserAccessMode {
        let mut max = None;
        for id in iter::once(id).chain(self.ancestors(id).iter().copied()) {
            let file = self.get_by_id(id).unwrap();
            for share in &file.shares {
                if share.shared_with == account.username {
                    let mode = match share.mode {
                        ShareMode::Write => UserAccessMode::Write,
                        ShareMode::Read => UserAccessMode::Read,
                    };
                    max = Some(max.map_or(mode, |m: UserAccessMode| m.max(mode)));
                }
            }
        }
        max.unwrap_or(UserAccessMode::Owner)
    }
}

impl FilesExt for [File] {
    fn root(&self) -> &File {
        for file in self {
            if file.is_root() {
                return file;
            }
        }
        unreachable!("unable to find root in metadata list")
    }

    fn get_by_id(&self, id: Uuid) -> Option<&File> {
        self.iter().find(|f| f.id == id)
    }

    fn iter_files(&self) -> impl Iterator<Item = &File> {
        self.iter()
    }

    fn children(&self, id: Uuid) -> Vec<&File> {
        let mut children: Vec<_> = self
            .iter()
            .filter(|f| f.parent == id && f.parent != f.id)
            .collect();
        children.sort_by(|a, b| match (a.file_type, b.file_type) {
            (FileType::Folder, FileType::Document) => Ordering::Less,
            (FileType::Document, FileType::Folder) => Ordering::Greater,
            (_, _) => a.name.cmp(&b.name),
        });
        children
    }
}

impl FilesExt for Vec<File> {
    fn root(&self) -> &File {
        self.as_slice().root()
    }

    fn get_by_id(&self, id: Uuid) -> Option<&File> {
        self.as_slice().get_by_id(id)
    }

    fn children(&self, id: Uuid) -> Vec<&File> {
        self.as_slice().children(id)
    }

    fn descendents(&self, id: Uuid) -> Vec<&File> {
        self.as_slice().descendents(id)
    }

    fn iter_files(&self) -> impl Iterator<Item = &File> {
        self.as_slice().iter_files()
    }

    fn path(&self, id: Uuid) -> String {
        self.as_slice().path(id)
    }

    fn by_path(&self, path: &str) -> Option<&File> {
        self.as_slice().by_path(path)
    }
}

impl FilesExt for FileCache {
    fn root(&self) -> &File {
        &self.root
    }

    fn get_by_id(&self, id: Uuid) -> Option<&File> {
        self.by_id.get(&id).map(|&i| &self.rows[i as usize])
    }

    fn children(&self, id: Uuid) -> Vec<&File> {
        let start = self.rows.partition_point(|f| f.parent < id);
        let end = self.rows.partition_point(|f| f.parent <= id);
        self.rows[start..end]
            .iter()
            .filter(|f| f.id != id)
            .collect()
    }

    fn iter_files(&self) -> impl Iterator<Item = &File> {
        self.all_files()
    }
}

/// A file name with its final extension removed (`note.svg` → `note`). Names
/// with no extension, a leading dot, or a trailing dot are returned unchanged.
pub fn strip_ext(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => &name[..i],
        _ => name,
    }
}

/// The title a file answers to in a wikilink, as in Obsidian: a note's name
/// without `.md`, any other file's full name (`chart.png`).
pub fn wiki_title(name: &str) -> &str {
    match name.len().checked_sub(3) {
        Some(i) if i > 0 && name.is_char_boundary(i) && name[i..].eq_ignore_ascii_case(".md") => {
            &name[..i]
        }
        _ => name,
    }
}

/// Whether a wikilink title matches a file name: its full name, or its
/// [`wiki_title`]. Case-insensitive.
pub fn title_matches(name: &str, title: &str) -> bool {
    name.eq_ignore_ascii_case(title) || wiki_title(name).eq_ignore_ascii_case(title)
}

/// A wikilink title as it is written: `[`, `]`, and `|` would end it or
/// split it, so they are escaped.
pub(crate) fn escape_wiki_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for c in title.chars() {
        if matches!(c, '[' | ']' | '|' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Percent-encodes characters that would end or change a CommonMark bare
/// link destination, or be read back as an escape or a fragment.
pub fn encode_link_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '#' => out.push_str("%23"),
            '%' => out.push_str("%25"),
            _ => out.push(c),
        }
    }
    out
}

/// Whether `url` leads with a URL scheme (`https:`, `mailto:`, `obsidian:`…).
fn has_scheme(url: &str) -> bool {
    let scheme = url.split_once(':').map_or("", |(scheme, _)| scheme);
    scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
}

/// The file an id link names: `lb://<uuid>`, or the external URL
/// `http(s)://<host>/open/<uuid>`. A query or `#fragment` rides along.
pub fn link_id(url: &str) -> Option<Uuid> {
    let url = url.split(['#', '?']).next()?;
    let id = match strip_prefix_ci(url, "lb://") {
        Some(id) => id,
        None => {
            let rest =
                strip_prefix_ci(url, "https://").or_else(|| strip_prefix_ci(url, "http://"))?;
            rest.split_once('/')?.1.strip_prefix("open/")?
        }
    };
    Uuid::parse_str(id.trim_end_matches('/')).ok()
}

/// Whether an unresolved destination is a URL for the host to open: a
/// `#fragment`, a `scheme://` URL other than Lockbook's own, or `mailto:`,
/// `tel:`, or `sms:`. A bare `word:` is a file name (`Q3:plan.md`). An
/// external URL from `app.lockbook.net` names a file you can't see, so it
/// isn't one.
pub(crate) fn web_link(url: &str) -> bool {
    if url.starts_with('#') {
        return true;
    }
    let Some((scheme, rest)) = url.split_once(':') else { return false };
    let is_scheme = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    let opaque = ["mailto", "tel", "sms"]
        .iter()
        .any(|s| scheme.eq_ignore_ascii_case(s));
    let official = strip_prefix_ci(rest, "//app.lockbook.net/").is_some() && link_id(url).is_some();
    is_scheme
        && (rest.starts_with("//") || opaque)
        && !scheme.eq_ignore_ascii_case("lb")
        && !official
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

/// A lockbook path split into its non-empty segments — the shared step
/// behind every path-boundary comparison here (and `chat::tools::in_scope`):
/// segment-vector equality is immune to the sibling-prefix trap raw string
/// slicing invites (`/notes` is not a prefix-match for `/notes2/a.md` once
/// paths are segments rather than characters).
pub fn path_segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

pub fn relative_path(from: &str, to: &str) -> String {
    if from == to {
        if from.ends_with('/') {
            return "./".to_string();
        } else {
            return ".".to_string();
        }
    }

    let from_parts = path_segments(from);
    let to_parts = path_segments(to);

    let num_common = from_parts
        .iter()
        .zip(to_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut result = "../".repeat(from_parts.len() - num_common);
    for part in &to_parts[num_common..] {
        result.push_str(part);
        result.push('/');
    }
    if !to.ends_with('/') {
        result.pop();
    }
    result
}

pub fn canonicalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for component in path_segments(path) {
        match component {
            ".." => {
                parts.pop();
            }
            "." => {}
            _ => parts.push(component),
        }
    }
    parts.join("/")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use lb_rs::model::file_metadata::FileType;

    #[test]
    fn path_segments_tests() {
        assert_eq!(path_segments("/"), Vec::<&str>::new());
        assert_eq!(path_segments("/a"), vec!["a"]);
        assert_eq!(path_segments("/a/"), vec!["a"]);
        assert_eq!(path_segments("/a/b/c"), vec!["a", "b", "c"]);
        // A doubled slash collapses rather than yielding an empty segment.
        assert_eq!(path_segments("/a//b"), vec!["a", "b"]);
    }

    #[test]
    fn relative_path_tests() {
        assert_eq!(relative_path("/a/b/c", "/a/b/c"), ".");
        assert_eq!(relative_path("/a/b/c", "/a/b/c/d"), "d");
        assert_eq!(relative_path("/a/b/c", "/a/b/c/d/e"), "d/e");
        assert_eq!(relative_path("/a/b/c", "/a/b/c/d/e/f"), "d/e/f");

        assert_eq!(relative_path("/a/b/c", "/a/b/d"), "../d");
        assert_eq!(relative_path("/a/b/c", "/a/b/d/e"), "../d/e");
        assert_eq!(relative_path("/a/b/c", "/a/b/d/e/f"), "../d/e/f");

        assert_eq!(relative_path("/a/b/c", "/a/d"), "../../d");
        assert_eq!(relative_path("/a/b/c", "/a/d/e"), "../../d/e");
        assert_eq!(relative_path("/a/b/c", "/a/d/e/f"), "../../d/e/f");

        assert_eq!(relative_path("/a/b/c", "/d"), "../../../d");
        assert_eq!(relative_path("/a/b/c", "/d/e"), "../../../d/e");
        assert_eq!(relative_path("/a/b/c", "/d/e/f"), "../../../d/e/f");

        // to folders
        assert_eq!(relative_path("/a/b/c", "/a/b/c/d/"), "d/");
        assert_eq!(relative_path("/a/b/c", "/a/b/c/d/e/"), "d/e/");
        assert_eq!(relative_path("/a/b/c", "/a/b/c/d/e/f/"), "d/e/f/");

        assert_eq!(relative_path("/a/b/c", "/a/b/"), "../");
        assert_eq!(relative_path("/a/b/c", "/a/b/d/"), "../d/");
        assert_eq!(relative_path("/a/b/c", "/a/b/d/e/"), "../d/e/");
        assert_eq!(relative_path("/a/b/c", "/a/b/d/e/f/"), "../d/e/f/");

        assert_eq!(relative_path("/a/b/c", "/a/"), "../../");
        assert_eq!(relative_path("/a/b/c", "/a/d/"), "../../d/");
        assert_eq!(relative_path("/a/b/c", "/a/d/e/"), "../../d/e/");
        assert_eq!(relative_path("/a/b/c", "/a/d/e/f/"), "../../d/e/f/");

        assert_eq!(relative_path("/a/b/c", "/"), "../../../");
        assert_eq!(relative_path("/a/b/c", "/d/"), "../../../d/");
        assert_eq!(relative_path("/a/b/c", "/d/e/"), "../../../d/e/");
        assert_eq!(relative_path("/a/b/c", "/d/e/f/"), "../../../d/e/f/");
    }

    fn file(id: Uuid, parent: Uuid, name: &str, file_type: FileType) -> File {
        File {
            id,
            parent,
            name: name.to_string(),
            file_type,
            last_modified: 0,
            last_modified_by: Default::default(),
            owner: Default::default(),
            shares: vec![],
            size_bytes: 0,
        }
    }

    fn tree() -> Vec<File> {
        let root = Uuid::new_v4();
        let folder = Uuid::new_v4();
        let doc = Uuid::new_v4();
        vec![
            file(root, root, "root", FileType::Folder),
            file(folder, root, "notes", FileType::Folder),
            file(doc, folder, "meeting.md", FileType::Document),
        ]
    }

    #[test]
    fn path_document() {
        let files = tree();
        let doc = files.iter().find(|f| f.name == "meeting.md").unwrap();
        assert_eq!(files.path(doc.id), "/notes/meeting.md");
    }

    #[test]
    fn path_folder() {
        let files = tree();
        let folder = files.iter().find(|f| f.name == "notes").unwrap();
        assert_eq!(files.path(folder.id), "/notes/");
    }

    #[test]
    fn by_path_roundtrip() {
        let files = tree();
        let doc = files.iter().find(|f| f.name == "meeting.md").unwrap();
        let found = files.by_path("/notes/meeting.md").unwrap();
        assert_eq!(found.id, doc.id);
    }

    #[test]
    fn by_path_missing() {
        let files = tree();
        assert!(files.by_path("/notes/nonexistent.md").is_none());
    }

    /// A title with folders in it is walked from the note's folder, else
    /// names a document whose folders end with them, nearest first; `/` leads
    /// from the scope's top.
    #[test]
    fn path_titles_are_trailing_paths() {
        use crate::test_utils::files::{at, cache, tree};
        let files = tree(
            "alice",
            &[
                "/Plan.md",
                "/project/ @bob",
                "/project/notes.md",
                "/project/design/Plan.md",
                "/project/design/sub/notes.md",
                "/project/research/Plan.md",
                "/project/research/old/Plan.md",
            ],
        );
        let note = at(&files, "/project/notes.md");
        let deep = at(&files, "/project/design/sub/notes.md");
        let cache = cache(files);
        let design = at(&cache, "/project/design/Plan.md");
        let research = at(&cache, "/project/research/Plan.md");
        let old = at(&cache, "/project/research/old/Plan.md");
        let resolve = |title: &str, from| cache.resolve_wikilink(title, from);

        assert_eq!(resolve("Plan", note), None); // a tie
        assert_eq!(resolve("design/Plan", deep), Some(design));
        assert_eq!(resolve("research/Plan", note), Some(research)); // nearer than old/
        assert_eq!(resolve("old/Plan", note), Some(old));
        assert_eq!(resolve("research/old/Plan", deep), Some(old));
        assert_eq!(resolve("/design/Plan", deep), Some(design));
        assert_eq!(resolve("../Plan", deep), Some(design));
        assert_eq!(resolve("./Plan", deep), None);
        // the top has no name, and `/` is the scope's top: these miss the
        // scope and reach the reader's own files instead
        let locate = |title: &str, from| cache.locate_wikilink(title, from);
        assert_eq!(locate("project/design/Plan", deep), Some((design, Reach::Reader)));
        assert_eq!(locate("/Plan", deep), Some((at(&cache, "/Plan.md"), Reach::Reader)));
    }

    #[test]
    fn clustered_children_folders_then_name() {
        let root = Uuid::from_u128(1);
        let cache = FileCache::from_owned_and_shared(
            file(root, root, "root", FileType::Folder),
            [
                file(root, root, "root", FileType::Folder),
                file(Uuid::from_u128(2), root, "zeta.md", FileType::Document),
                file(Uuid::from_u128(3), root, "alpha", FileType::Folder),
                file(Uuid::from_u128(4), root, "beta.md", FileType::Document),
                file(Uuid::from_u128(5), root, "mid", FileType::Folder),
            ],
            [],
        );
        let names: Vec<&str> = cache
            .children(root)
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(names, ["alpha", "mid", "beta.md", "zeta.md"]);
        assert!(cache.children(root).iter().all(|f| f.id != root));
    }

    #[test]
    fn insert_created_file_keeps_cluster_order() {
        let root = Uuid::from_u128(1);
        let mut cache = FileCache::from_owned_and_shared(
            file(root, root, "root", FileType::Folder),
            [
                file(root, root, "root", FileType::Folder),
                file(Uuid::from_u128(2), root, "b.md", FileType::Document),
            ],
            [],
        );
        cache.insert_created_file(file(Uuid::from_u128(3), root, "a", FileType::Folder));
        cache.insert_created_file(file(Uuid::from_u128(4), root, "c.md", FileType::Document));
        let names: Vec<&str> = cache
            .children(root)
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(names, ["a", "b.md", "c.md"]);
        assert_eq!(cache.get_by_id(Uuid::from_u128(3)).unwrap().name, "a");
        assert_eq!(cache.get_by_id(Uuid::from_u128(4)).unwrap().name, "c.md");
    }

    /// Alice's tree: `/personal/Budget.md`, `/finance/budget.md`, and
    /// `/project/` shared with Bob, holding `Budget.md` and `design/plan.md`.
    pub(crate) fn alice() -> (FileCache, [Uuid; 6]) {
        let [root, personal, finance, project, design, note] =
            [1, 2, 3, 4, 5, 6].map(Uuid::from_u128);
        let [own_budget, finance_budget, project_budget] = [7, 8, 9].map(Uuid::from_u128);
        let mut shared = file(project, root, "project", FileType::Folder);
        shared.shares.push(lb_rs::model::file::Share {
            mode: ShareMode::Write,
            shared_by: "alice".into(),
            shared_with: "bob".into(),
        });
        let files = [
            file(root, root, "root", FileType::Folder),
            file(personal, root, "personal", FileType::Folder),
            file(own_budget, personal, "Budget.md", FileType::Document),
            file(finance, root, "finance", FileType::Folder),
            file(finance_budget, finance, "budget.md", FileType::Document),
            shared,
            file(project_budget, project, "Budget.md", FileType::Document),
            file(design, project, "design", FileType::Folder),
            file(note, design, "plan.md", FileType::Document),
        ]
        .map(|mut f| {
            f.owner = "alice".into();
            f
        });
        let cache = FileCache::from_owned_and_shared(files[0].clone(), files, []);
        (cache, [note, project, own_budget, finance_budget, project_budget, root])
    }

    #[test]
    fn shared_note_resolves_in_its_scope() {
        let (cache, [note, project, own_budget, finance_budget, project_budget, root]) = alice();
        assert_eq!(cache.scope_top(note), project);
        assert_eq!(cache.resolve_wikilink("Budget", note), Some(project_budget));
        assert_eq!(cache.locate_link("../Budget.md", note), Some((project_budget, Reach::Scope)));
        assert_eq!(
            cache.locate_link("../../personal/Budget.md", note),
            Some((own_budget, Reach::Reader))
        );
        assert_eq!(cache.locate_wikilink("Budget", note), Some((project_budget, Reach::Scope)));
        assert_eq!(
            cache.locate_wikilink("../../personal/Budget", note),
            Some((own_budget, Reach::Reader))
        );
        assert_eq!(
            cache.locate_link("/finance/budget.md", note),
            Some((finance_budget, Reach::Reader))
        );
        let id = format!("lb://{own_budget}");
        assert_eq!(cache.locate_link(&id, note), Some((own_budget, Reach::Reader)));
        assert_eq!(cache.scope_top(own_budget), root);
        assert_eq!(
            cache.locate_link("/finance/budget.md", own_budget),
            Some((finance_budget, Reach::Scope))
        );
    }

    #[test]
    fn link_ids_and_web_links() {
        let id = Uuid::from_u128(42);
        for url in [
            format!("lb://{id}"),
            format!("https://app.lockbook.net/open/{id}"),
            format!("HTTPS://app.lockbook.net/open/{id}/#intro"),
            format!("http://localhost:8000/open/{id}?x=1"),
        ] {
            assert_eq!(link_id(&url), Some(id), "{url}");
        }
        assert_eq!(link_id(&format!("https://app.lockbook.net/{id}")), None);
        assert_eq!(link_id("lb://not-an-id"), None);

        assert!(web_link("https://example.com"));
        assert!(web_link("mailto:a@b.c"));
        assert!(web_link("tel:123"));
        assert!(web_link("file:///Users/me/a.txt"));
        assert!(!web_link("Q3:plan.md"));
        assert!(web_link("#heading"));
        assert!(web_link(&format!("http://localhost:8000/open/{id}")));
        assert!(!web_link(&format!("https://app.lockbook.net/open/{id}")));
        assert!(!web_link(&format!("lb://{id}")));
        assert!(!web_link("notes/plan.md"));
        assert!(!web_link("/notes/plan.md"));
    }
}
