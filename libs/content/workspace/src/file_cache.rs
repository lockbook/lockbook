use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::iter;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

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

pub enum ResolvedLink {
    File(Uuid),
    External(String),
}

pub(crate) type UuidMap<V> = HashMap<Uuid, V, UuidIdentityHasherBuilder>;

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
    /// Documents by what a wikilink may call them: lowercased name, and name
    /// less its extension.
    by_title: HashMap<String, Vec<Uuid>>,
    pub shared_roots: Vec<File>,
    pub suggested: Vec<Uuid>,
    pub size_bytes_recursive: UuidMap<u64>,
    pub last_modified_recursive: UuidMap<u64>,
    /// A number no other state of any cache has had: the key for whatever
    /// is derived from the tree.
    pub stamp: u64,
}

fn stamp() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, AtomicOrdering::Relaxed)
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

fn index_title(by_title: &mut HashMap<String, Vec<Uuid>>, file: &File) {
    if !file.is_document() {
        return;
    }
    let name = file.name.to_ascii_lowercase();
    let stem = strip_ext(&name);
    if stem != name {
        by_title.entry(stem.to_string()).or_default().push(file.id);
    }
    by_title.entry(name).or_default().push(file.id);
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
            stamp: stamp(),
            shared_roots: vec![],
            rows: vec![root],
            by_id: [(root_id, 0)].into_iter().collect(),
            by_title: Default::default(),
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
        // A share inside another share comes once where it is placed and
        // again under the outer one; the first, the placed one, is kept.
        let mut seen = uuid_map();
        let mut rows: Vec<File> = files
            .into_iter()
            .filter(|f| seen.insert(f.id, ()).is_none())
            .collect();
        if !rows.iter().any(|f| f.id == root.id) {
            rows.push(root.clone());
        }
        rows.sort_by(cmp_cluster);
        let by_id = index_rows(&rows);
        let mut by_title = HashMap::new();
        for file in &rows {
            index_title(&mut by_title, file);
        }
        Self {
            root,
            rows,
            by_id,
            by_title,
            shared_roots,
            suggested,
            size_bytes_recursive: uuid_map(),
            last_modified_recursive: uuid_map(),
            stamp: stamp(),
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
        index_title(&mut self.by_title, &file);
        self.rows.insert(idx, file);
        for i in idx..self.rows.len() {
            self.by_id.insert(self.rows[i].id, i as u32);
        }

        self.size_bytes_recursive.insert(file_id, file_size);
        self.last_modified_recursive.insert(file_id, file_modified);
        self.stamp = stamp();

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

    /// The file in folder `id` named exactly `name`.
    fn child_named(&self, id: Uuid, name: &str) -> Option<&File> {
        self.children(id).into_iter().find(|f| f.name == name)
    }

    /// Documents the last segment of a wikilink title names: by full name,
    /// or by name less its extension. Case-insensitive.
    fn named(&self, title: &str) -> Vec<&File> {
        self.iter_files()
            .filter(|f| f.is_document() && title_matches(&f.name, title))
            .collect()
    }

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
    /// else the top of its tree. Every reader of the note sees that file and
    /// holds everything under it, whichever of their shares they placed, so
    /// links resolved inside it mean the same to all of them.
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
        let mut current = id;
        loop {
            if current == scope {
                return true;
            }
            let Some(file) = self.get_by_id(current) else { return false };
            if file.is_root() {
                return false;
            }
            current = file.parent;
        }
    }

    /// Whether `author` could see `id`: they own it, or it sits in a tree
    /// shared with them.
    fn within_reach(&self, author: &str, id: Uuid) -> bool {
        let Some(file) = self.get_by_id(id) else { return false };
        if file.owner == author {
            return true;
        }
        iter::once(id).chain(self.ancestors(id)).any(|id| {
            self.get_by_id(id)
                .is_some_and(|f| f.shares.iter().any(|s| s.shared_with == author))
        })
    }

    /// Walks `rel` from `from` without leaving `scope`: `..` at the scope's
    /// top fails, as does starting outside it.
    fn resolve_in_scope(&self, scope: Uuid, from: Uuid, rel: &str) -> Option<&File> {
        self.walk_in_scope(scope, from, rel, true)
    }

    /// [`Self::resolve_in_scope`]. When not `exact`, a segment of `rel` names
    /// a file whatever its case and the spaces around it.
    fn walk_in_scope(&self, scope: Uuid, from: Uuid, rel: &str, exact: bool) -> Option<&File> {
        if !self.in_scope(scope, from) {
            return None;
        }
        let mut current = from;
        for component in rel.split('/') {
            match component.trim() {
                "" | "." => {}
                ".." => {
                    if current == scope {
                        return None;
                    }
                    current = self.get_by_id(current)?.parent;
                }
                _ if exact => current = self.child_named(current, component)?.id,
                segment => {
                    let named = |f: &&File| f.name.eq_ignore_ascii_case(segment);
                    current = self.children(current).into_iter().find(named)?.id;
                }
            }
        }
        self.get_by_id(current)
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

    /// Resolves the destination of a link or image written in `note`.
    ///
    /// - relative path — walked from the note's folder
    /// - absolute path (`/foo`) — anchored at the top of the note's scope
    /// - `lb://uuid`, or the external URL of a file here — any document the
    ///   note's owner could see
    /// - any other URL (`https:`, `mailto:`, `tel:`…), `#heading` — returned
    ///   as `External(url)`
    ///
    /// Paths never leave the note's scope ([`Self::scope_top`]). Only
    /// documents resolve to `File`; folders are treated as broken. A
    /// `#fragment` rides along without changing what resolves. Returns None
    /// for an internal destination that doesn't resolve.
    fn resolve_link(&self, url: &str, note: Uuid) -> Option<ResolvedLink> {
        self.resolve_link_within(url, note, self.scope_top(note))
    }

    /// [`Self::resolve_link`] with `scope` standing in for the note's own.
    fn resolve_link_within(&self, url: &str, note: Uuid, scope: Uuid) -> Option<ResolvedLink> {
        let leads = |scheme: &str| leads_with(url, scheme).is_some();
        let web = ["http://", "https://", "mailto:"].into_iter().any(leads);
        if let Some(id) = link_id(url) {
            let found = self.get_by_id(note).zip(self.get_by_id(id));
            if let Some((source, file)) = found {
                let reachable = self.in_scope(scope, id) || self.within_reach(&source.owner, id);
                return (file.is_document() && reachable).then_some(ResolvedLink::File(id));
            }
        }
        if leads("lb:") {
            return None; // ours to open, and it names nothing here
        }
        if web || url.starts_with('#') {
            return Some(ResolvedLink::External(url.to_string()));
        }

        // a file named `Re: plan.md` reads as a URL but is a path first
        let external = has_scheme(url).then(|| ResolvedLink::External(url.to_string()));
        let Some(source) = self.get_by_id(note) else { return external };
        let resolve = |path: &str| {
            let decoded = decode(path)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| path.to_string());
            let file = if decoded.starts_with('/') {
                self.resolve_in_scope(scope, scope, &canonicalize(&decoded))?
            } else {
                self.resolve_in_scope(scope, source.parent, &decoded)?
            };
            file.is_document().then_some(ResolvedLink::File(file.id))
        };
        resolve(url)
            .or_else(|| resolve(url.split_once('#')?.0))
            .or(external)
    }

    /// The destination a link in `note` to `target` should be written with:
    /// a relative path inside the note's scope, `lb://` out of it.
    fn link_destination(&self, target: Uuid, note: Uuid) -> String {
        let scope = self.scope_top(note);
        let from = self.get_by_id(note).map(|f| f.parent).unwrap_or(note);
        if self.in_scope(scope, from) && self.in_scope(scope, target) {
            let path = encode_link_path(&relative_path(&self.path(from), &self.path(target)));
            // `Re: plan.md` would read as a URL
            if has_scheme(&path) { format!("./{path}") } else { path }
        } else {
            format!("lb://{target}")
        }
    }

    /// The shortest wikilink title that means `target` anywhere in `note`'s
    /// scope: its stem, else with enough folders in front, else its full name
    /// likewise. A document whose every trailing path also ends a deeper
    /// one's gets its whole path, from the scope's top. None outside the
    /// scope.
    fn wikilink_title(&self, target: Uuid, note: Uuid) -> Option<String> {
        let scope = self.scope_top(note);
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
                .named(last)
                .into_iter()
                .filter(|f| f.id != scope && self.in_scope(scope, f.id))
                .filter(|f| self.trails(scope, f, &path))
                .count();
            path.push(last);
            (matches == 1).then(|| escape_wiki_title(&path.join("/")))
        };
        let (stem, name) = (strip_ext(&file.name), file.name.as_str());
        [stem, name]
            .into_iter()
            .flat_map(|last| (0..=dirs.len()).map(move |n| (last, n)))
            .find_map(|(last, n)| unique(last, &dirs[..n]))
            .or_else(|| {
                let beside = self.children(file.parent);
                let stems = beside
                    .iter()
                    .filter(|f| f.is_document() && title_matches(&f.name, stem));
                let last = if stems.count() == 1 { stem } else { name };
                let dirs = dirs.iter().rev().map(|d| format!("{d}/"));
                Some(escape_wiki_title(&format!("/{}{last}", dirs.collect::<String>())))
            })
    }

    /// Resolves a wikilink title written in `note` to a document in the
    /// note's scope ([`Self::scope_top`]).
    ///
    /// Extensions are optional in the link, never stripped from the file:
    /// `note` matches a document named `note.md`, `note.svg`, or `note`, while
    /// `note.svg` matches only the exact name. Case and the spaces around a
    /// segment don't count.
    ///
    /// - bare titles match any document in the scope.
    /// - path titles (`folder/note`) match documents whose path ends that way.
    /// - titles with a leading `/`, `.`, or `..` are paths, as in
    ///   [`Self::resolve_link`].
    ///
    /// The nearest match to the note's folder wins, and of several equally
    /// near, one named exactly as the title. Only documents match. Returns
    /// None when nothing matches or the nearest still tie — adding an
    /// extension or a path disambiguates.
    fn resolve_wikilink(&self, title: &str, note: Uuid) -> Option<Uuid> {
        match self.wikilink_matches(title, note)[..] {
            [file] => Some(file.id),
            _ => None,
        }
    }

    /// The documents a wikilink title could mean: one when it resolves, none
    /// when nothing matches, several when the nearest matches tie.
    fn wikilink_matches(&self, title: &str, note: Uuid) -> Vec<&File> {
        self.wikilink_matches_within(title, note, self.scope_top(note))
    }

    /// [`Self::wikilink_matches`] with `scope` standing in for the note's own.
    fn wikilink_matches_within(&self, title: &str, note: Uuid, scope: Uuid) -> Vec<&File> {
        self.wikilink_split(title, note, scope).0
    }

    /// The documents `title` could mean in `scope`, and how much of it names
    /// them; the rest is a fragment. A `#` starts one unless a name has it:
    /// the file is the longest title before one that matches.
    fn wikilink_split(&self, title: &str, note: Uuid, scope: Uuid) -> (Vec<&File>, usize) {
        let Some(source) = self.get_by_id(note) else { return (vec![], title.len()) };
        let cuts = title.rmatch_indices('#').map(|(at, _)| at);
        for at in std::iter::once(title.len()).chain(cuts) {
            if at == 0 && title.starts_with('#') {
                return (vec![source], 0); // `[[#heading]]` is a place in the note itself
            }
            let named = self.wikilink_matches_exactly(&title[..at], note, scope);
            if !named.is_empty() {
                return (named, at);
            }
        }
        (vec![], title.len())
    }

    /// [`Self::wikilink_split`] for a title with no fragment to set aside.
    fn wikilink_matches_exactly(&self, title: &str, note: Uuid, scope: Uuid) -> Vec<&File> {
        let Some(source) = self.get_by_id(note) else { return vec![] };
        let from = source.parent;
        let (dir, last) = title.rsplit_once('/').unwrap_or(("", title));
        let last = last.trim();

        let candidates: Vec<&File> =
            if title.starts_with('/') || dir.split('/').any(|s| matches!(s.trim(), "." | "..")) {
                let start = if title.starts_with('/') { scope } else { from };
                let Some(folder) = self.walk_in_scope(scope, start, dir, false) else {
                    return vec![];
                };
                self.children(folder.id)
                    .into_iter()
                    .filter(|f| f.is_document() && title_matches(&f.name, last))
                    .collect()
            } else {
                // The scope top's name is each reader's to choose, so it never
                // takes part in a match.
                let dirs: Vec<&str> = path_segments(dir).into_iter().map(str::trim).collect();
                self.named(last)
                    .into_iter()
                    .filter(|f| f.id != scope && self.in_scope(scope, f.id))
                    .filter(|f| self.trails(scope, f, &dirs))
                    .collect()
            };

        let distance = |f: &File| self.folder_distance(from, f.id);
        let Some(nearest) = candidates.iter().map(|f| distance(f)).min() else { return vec![] };
        let nearest: Vec<&File> = candidates
            .into_iter()
            .filter(|f| distance(f) == nearest)
            .collect();
        // Of those, a name the title spells in full outranks one it shortens.
        let exact = |f: &&File| f.name.eq_ignore_ascii_case(last);
        if nearest.iter().any(exact) {
            nearest.into_iter().filter(exact).collect()
        } else {
            nearest
        }
    }

    /// The document a link would reach were its note's scope its owner's
    /// whole tree: where it pointed before a share narrowed the scope. Only
    /// in that owner's view, and only a document they can see.
    fn beyond_scope(&self, dest: &str, wikilink: bool, note: Uuid) -> Option<Uuid> {
        // what lay beyond the scope is there only in the owner's own tree
        let owner = &self.get_by_id(note)?.owner;
        let tree = self.tree_root(note);
        if tree != self.root().id || *owner != self.root().owner {
            return None;
        }
        let id = if wikilink {
            match self.wikilink_matches_within(dest, note, tree)[..] {
                [file] => file.id,
                _ => return None,
            }
        } else {
            match self.resolve_link_within(dest, note, tree)? {
                ResolvedLink::File(id) => id,
                ResolvedLink::External(_) => return None,
            }
        };
        self.within_reach(owner, id).then_some(id)
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

    /// Where the note an unmatched wikilink title names would be created: an
    /// existing folder, then the names to create under it, the document last.
    /// A path title is walked from the note's folder (`/` from the scope's
    /// top). None for a title that names no file, or when the note's folder
    /// is outside its scope.
    fn wikilink_placement(&self, title: &str, note: Uuid) -> Option<(Uuid, Vec<String>)> {
        let from = self.get_by_id(note)?.parent;
        let scope = self.scope_top(note);
        let title = title.split_once('#').map_or(title, |(title, _)| title);
        let mut names: Vec<String> = title
            .strip_prefix('/')
            .unwrap_or(title)
            .split('/')
            .map(|s| s.trim().to_string())
            .collect();
        if !self.in_scope(scope, from)
            || names.iter().any(|s| s.is_empty() || s == "." || s == "..")
        {
            return None;
        }
        let last = names.last_mut()?;
        if !has_extension(last) {
            last.push_str(".md");
        }

        let mut current = if title.starts_with('/') { scope } else { from };
        while names.len() > 1 {
            let children = self.children(current);
            let existing = |f: &&File| f.is_folder() && f.name.eq_ignore_ascii_case(&names[0]);
            let Some(folder) = children.into_iter().find(existing) else { break };
            current = folder.id;
            names.remove(0);
        }
        Some((current, names))
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

    fn resolve_link(&self, url: &str, note: Uuid) -> Option<ResolvedLink> {
        self.as_slice().resolve_link(url, note)
    }

    fn resolve_wikilink(&self, title: &str, note: Uuid) -> Option<Uuid> {
        self.as_slice().resolve_wikilink(title, note)
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

    fn child_named(&self, id: Uuid, name: &str) -> Option<&File> {
        // rows are in `cmp_cluster` order: a folder's folders, then its
        // documents; the root is its own parent and may share a name
        [false, true].into_iter().find_map(|document| {
            let ahead = |f: &File| (f.parent, f.is_document(), &*f.name).cmp(&(id, document, name));
            let start = self.rows.partition_point(|f| ahead(f).is_lt());
            let named = self.rows[start..].iter().take_while(|f| ahead(f).is_eq());
            named.into_iter().find(|f| f.id != id)
        })
    }

    fn iter_files(&self) -> impl Iterator<Item = &File> {
        self.all_files()
    }

    fn named(&self, title: &str) -> Vec<&File> {
        let ids = self.by_title.get(&title.to_ascii_lowercase());
        ids.into_iter()
            .flatten()
            .filter_map(|id| self.get_by_id(*id))
            .collect()
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

/// Whether a name a wikilink would create ends in the extension of a kind of
/// file made empty and then written in. `Node.js`, `v1.2 notes`, and
/// `Mr. Smith` are titles of notes.
fn has_extension(name: &str) -> bool {
    let ext = name.rsplit_once('.').map_or("", |(_, ext)| ext);
    ["md", "svg", "txt"]
        .iter()
        .any(|e| ext.eq_ignore_ascii_case(e))
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

/// Whether a wikilink title matches a file name: an exact match, or a match
/// once the file's extension is dropped. Case-insensitive.
pub fn title_matches(name: &str, title: &str) -> bool {
    name.eq_ignore_ascii_case(title) || strip_ext(name).eq_ignore_ascii_case(title)
}

/// A lockbook path split into its non-empty segments — the shared step
/// behind every path-boundary comparison here (and `chat::tools::in_scope`):
/// segment-vector equality is immune to the sibling-prefix trap raw string
/// slicing invites (`/notes` is not a prefix-match for `/notes2/a.md` once
/// paths are segments rather than characters).
pub fn path_segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
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
/// `https://<host>/open/<uuid>`, either with an optional `#fragment`.
pub fn link_id(url: &str) -> Option<Uuid> {
    // read as the platform's own opener reads it: any case, a query, a
    // trailing slash
    let url = url.split(['#', '?']).next()?;
    let id = match leads_with(url, "lb://") {
        Some(id) => id,
        None => {
            let rest = leads_with(url, "https://").or(leads_with(url, "http://"))?;
            rest.split_once('/')?.1.strip_prefix("open/")?
        }
    };
    Uuid::parse_str(id.trim_end_matches('/')).ok()
}

/// What follows `scheme` in a `url` that leads with it, in any case.
fn leads_with<'a>(url: &'a str, scheme: &str) -> Option<&'a str> {
    let lead = url.get(..scheme.len())?;
    lead.eq_ignore_ascii_case(scheme)
        .then(|| &url[scheme.len()..])
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
mod tests {
    use super::*;
    use crate::test_utils::files::{self as fixture, at};
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

    /// Alice's tree as she sees it, with `project/` shared with bob.
    fn shared_project() -> (Vec<File>, impl Fn(&str) -> Uuid) {
        let files = fixture::tree(
            "alice",
            &[
                "/budget.md",
                "/project/ @bob",
                "/project/plan.md",
                "/project/budget.md",
                "/project/design/plan.md",
                "/project/design/sketch.svg",
            ],
        );
        let lookup = files.clone();
        (files, move |path: &str| at(&lookup, path))
    }

    fn resolved(files: &Vec<File>, url: &str, note: Uuid) -> Option<Uuid> {
        match files.resolve_link(url, note)? {
            ResolvedLink::File(id) => Some(id),
            ResolvedLink::External(_) => None,
        }
    }

    #[test]
    fn urls_and_id_links() {
        let (files, id) = shared_project();
        let note = id("/project/plan.md");
        let budget = id("/project/budget.md");
        let external =
            |url: &str| matches!(files.resolve_link(url, note), Some(ResolvedLink::External(_)));

        for url in [
            "https://lockbook.net",
            "HTTPS://lockbook.net",
            "mailto:a@b.c",
            "tel:+15551234",
            "obsidian://open?vault=x",
            "#heading",
        ] {
            assert!(external(url), "{url}");
        }
        // the external URL of a file here is an id link; of any other, a web page
        assert_eq!(
            resolved(&files, &format!("https://app.lockbook.net/open/{budget}"), note),
            Some(budget)
        );
        assert_eq!(resolved(&files, &format!("lb://{budget}#totals"), note), Some(budget));
        assert!(external(&format!("https://app.lockbook.net/open/{}", Uuid::from_u128(99))));
        assert!(
            files
                .resolve_link(&format!("lb://{}", Uuid::from_u128(99)), note)
                .is_none()
        );
        // a fragment rides along; a `#` in a name still resolves
        assert_eq!(resolved(&files, "budget.md#totals", note), Some(budget));
        assert_eq!(files.resolve_wikilink("budget#totals", note), Some(budget));
        assert_eq!(files.resolve_wikilink("#totals", note), Some(note));
        assert_eq!(encode_link_path("C# (100%).md"), "C%23%20%28100%25%29.md");
    }

    #[test]
    fn paths_stay_in_the_scope() {
        let (files, id) = shared_project();
        let note = id("/project/design/plan.md");

        assert_eq!(files.scope_top(note), id("/project"));
        assert_eq!(files.scope_top(id("/budget.md")), files.root().id);

        assert_eq!(resolved(&files, "../budget.md", note), Some(id("/project/budget.md")));
        assert_eq!(resolved(&files, "../../budget.md", note), None);
        // `/` is the top of the scope, not the reader's root
        assert_eq!(resolved(&files, "/budget.md", note), Some(id("/project/budget.md")));
        assert_eq!(resolved(&files, "/project/budget.md", note), None);
        assert_eq!(
            resolved(&files, "/project/budget.md", id("/budget.md")),
            Some(id("/project/budget.md"))
        );
    }

    #[test]
    fn names_in_any_script_resolve() {
        let files = fixture::tree("alice", &["/заметки/résumé.md", "/заметки/日本語.md"]);
        let note = at(&files, "/заметки/résumé.md");
        let other = at(&files, "/заметки/日本語.md");

        assert_eq!(resolved(&files, "日本語.md", note), Some(other));
        assert_eq!(resolved(&files, "../заметки/日本語.md", note), Some(other));
        assert_eq!(resolved(&files, "нет.md", note), None);
        assert_eq!(files.resolve_wikilink("日本語", note), Some(other));
        let dest = files.link_destination(other, note);
        assert_eq!(resolved(&files, &dest, note), Some(other), "{dest}");
    }

    #[test]
    fn id_links_read_as_the_platform_opens_them() {
        let (files, id) = shared_project();
        let note = id("/project/plan.md");
        let budget = id("/project/budget.md");
        for url in [
            format!("lb://{budget}/"),
            format!("LB://{budget}?from=chat#totals"),
            format!("https://app.lockbook.net/open/{budget}?x=1"),
        ] {
            assert_eq!(resolved(&files, &url, note), Some(budget), "{url}");
        }
        // `lb:` is the app's to open: what it can't resolve goes nowhere
        for url in ["lb://nonsense", "lb:", "LB://sharedFiles?x"] {
            assert!(files.resolve_link(url, note).is_none(), "{url}");
        }
    }

    /// Bob holds both of alice's nested folders and placed the inner one,
    /// which hides the outer one above it from him. Both still agree on the
    /// scope of a note in the inner one.
    #[test]
    fn readers_agree_on_the_scope_however_shares_are_placed() {
        let alice =
            fixture::tree("alice", &["/outer/ @bob", "/outer/inner/ @bob", "/outer/inner/note.md"]);
        let (inner, note) = (at(&alice, "/outer/inner"), at(&alice, "/outer/inner/note.md"));
        let mut bob = fixture::tree("bob", &["/placed/"]);
        bob[1] = File { parent: bob[0].id, name: "placed".into(), ..alice[2].clone() };
        bob.push(alice[3].clone());

        assert_eq!(alice.scope_top(note), inner);
        assert_eq!(bob.scope_top(note), inner);
    }

    #[test]
    fn a_file_listed_twice_is_one_file() {
        let files = fixture::tree("bob", &["/placed/spec.md"]);
        let twice = files.iter().skip(1).cloned().collect::<Vec<_>>();
        let cache = FileCache::from_owned_and_shared(files[0].clone(), files.clone(), twice);
        let note = at(&files, "/placed/spec.md");
        assert_eq!(cache.children(at(&files, "/placed")).len(), 1);
        assert_eq!(cache.named("spec").len(), 1);
        assert_eq!(cache.wikilink_title(note, note).as_deref(), Some("spec"));
    }

    #[test]
    fn a_link_stopped_by_the_scope_still_says_where_it_pointed() {
        let (mut files, id) = shared_project();
        let note = id("/project/design/plan.md");
        let outside = id("/budget.md");

        assert_eq!(resolved(&files, "../../budget.md", note), None);
        assert_eq!(files.beyond_scope("../../budget.md", false, note), Some(outside));
        // `/` meant the owner's root before the share
        assert_eq!(
            files.beyond_scope("/project/budget.md", false, note),
            Some(id("/project/budget.md"))
        );
        assert_eq!(
            files.beyond_scope("project/budget", true, note),
            Some(id("/project/budget.md"))
        );
        assert_eq!(files.beyond_scope("../../nowhere.md", false, note), None);

        // never to a file the note's owner can't see
        files.iter_mut().find(|f| f.id == outside).unwrap().owner = "bob".into();
        assert_eq!(files.beyond_scope("../../budget.md", false, note), None);

        // nor from another's tree, where what is above the share is theirs:
        // bob placed the project beside a budget of his own
        let shifted = |id: Uuid| Uuid::from_u128(id.as_u128() + 100);
        let mut bobs: Vec<File> = fixture::tree("bob", &["/work/budget.md @alice"])
            .iter()
            .map(|f| File { id: shifted(f.id), parent: shifted(f.parent), ..f.clone() })
            .collect();
        let work = bobs[1].id;
        let placed = |f: &File| File {
            parent: if f.name == "project" { work } else { f.parent },
            ..f.clone()
        };
        let theirs = files
            .iter()
            .filter(|f| f.name != "alice" && f.id != outside);
        bobs.extend(theirs.map(placed));
        assert_eq!(resolved(&bobs, "../budget.md", note), Some(id("/project/budget.md")));
        assert_eq!(bobs.beyond_scope("../../budget.md", false, note), None);
    }

    #[test]
    fn id_links_stop_at_the_authors_reach() {
        let (mut files, id) = shared_project();
        let note = id("/project/plan.md");
        let outside = format!("lb://{}", id("/budget.md"));
        assert_eq!(resolved(&files, &outside, note), Some(id("/budget.md")));

        // bob's own file, seen from bob's side of the same note
        let mine = Uuid::from_u128(9);
        let mut private = file(mine, files[0].id, "private.md", FileType::Document);
        private.owner = "bob".into();
        files.push(private);
        assert_eq!(resolved(&files, &format!("lb://{mine}"), note), None);
    }

    #[test]
    fn wikilinks_resolve_nearest_first_in_the_scope() {
        let (files, id) = shared_project();
        let note = id("/project/design/sketch.svg");

        assert_eq!(files.resolve_wikilink("plan", note), Some(id("/project/design/plan.md")));
        assert_eq!(
            files.resolve_wikilink("plan", id("/project/budget.md")),
            Some(id("/project/plan.md"))
        );
        // the nearer of two, and never the one outside the share
        assert_eq!(files.resolve_wikilink("budget", note), Some(id("/project/budget.md")));
        assert_eq!(
            files.resolve_wikilink("design/plan", id("/project/budget.md")),
            Some(id("/project/design/plan.md"))
        );
        // the share's own name is the reader's to choose
        assert_eq!(files.resolve_wikilink("project/plan", note), None);
        // from outside the share, the nearer plan
        assert_eq!(files.resolve_wikilink("plan", id("/budget.md")), Some(id("/project/plan.md")));
        assert_eq!(
            files.resolve_wikilink("project/plan", id("/budget.md")),
            Some(id("/project/plan.md"))
        );
        // case and the spaces around a segment don't count
        let design_plan = Some(id("/project/design/plan.md"));
        assert_eq!(files.resolve_wikilink("Design / Plan", id("/project/budget.md")), design_plan);
        assert_eq!(files.resolve_wikilink("/DESIGN/plan", note), design_plan);
        assert_eq!(files.resolve_wikilink("../design/plan", id("/project/budget.md")), None);
        assert_eq!(files.resolve_wikilink("./design/Plan", id("/project/budget.md")), design_plan);
    }

    /// A file named exactly as a title doesn't take the link from a nearer
    /// one the title shortens: being nearer is the only way to capture it.
    #[test]
    fn a_nearer_match_beats_a_farther_exact_name() {
        let files = fixture::tree(
            "alice",
            &["/notes/budget.md", "/notes/log.md", "/budget", "/notes/sub/budget"],
        );
        let (log, near) = (at(&files, "/notes/log.md"), at(&files, "/notes/budget.md"));
        assert_eq!(files.resolve_wikilink("budget", log), Some(near));
        assert_eq!(files.resolve_wikilink("Budget.md", log), Some(near));
        // equally near, the one named in full
        let files = fixture::tree("alice", &["/notes/log.md", "/notes/budget.md", "/notes/budget"]);
        let log = at(&files, "/notes/log.md");
        assert_eq!(files.resolve_wikilink("budget", log), Some(at(&files, "/notes/budget")));
    }

    #[test]
    fn written_forms_resolve_back() {
        let (mut files, id) = shared_project();
        let (design, note) = (id("/project/design"), id("/project/plan.md"));
        let mut add = |n: u128, parent: Uuid, name: &str, file_type| {
            let mut f = file(Uuid::from_u128(n), parent, name, file_type);
            f.owner = "alice".into();
            files.push(f);
            Uuid::from_u128(n)
        };
        let colon = add(20, id("/project"), "Re: plan.md", FileType::Document);
        let sharp = add(21, id("/project"), "C# notes.md", FileType::Document);
        let x = add(22, design, "x", FileType::Folder);
        let xy = add(23, x, "y", FileType::Folder);
        let y = add(24, design, "y", FileType::Folder);
        let deep = add(25, xy, "Spec.md", FileType::Document);
        let shallow = add(26, y, "Spec.md", FileType::Document);

        let bracket = add(27, id("/project"), "Report [final].md", FileType::Document);
        let pipe = add(28, id("/project"), "Q3 | Plan.md", FileType::Document);

        // a title is read back as the editor reads it out of `[[…]]`
        let read = |title: &str| {
            use crate::tab::markdown_editor::MdRender;
            let (arena, source) = (comrak::Arena::new(), format!("[[{title}]]"));
            let root = comrak::parse_document(&arena, &source, &MdRender::comrak_options());
            let title = root
                .descendants()
                .find_map(|n| match &n.data.borrow().value {
                    comrak::nodes::NodeValue::WikiLink(link) => Some(link.url.clone()),
                    _ => None,
                });
            title.unwrap()
        };
        for target in [colon, sharp, deep, shallow, bracket, pipe] {
            let dest = files.link_destination(target, note);
            assert_eq!(resolved(&files, &dest, note), Some(target), "{dest}");
            let title = files.wikilink_title(target, note).unwrap();
            assert_eq!(files.resolve_wikilink(&read(&title), note), Some(target), "{title}");
        }
        assert_eq!(files.wikilink_title(deep, note).unwrap(), "x/y/Spec");
        assert_eq!(files.wikilink_title(shallow, note).unwrap(), "design/y/Spec");
        assert_eq!(files.wikilink_title(bracket, note).unwrap(), r"Report \[final\]");
        // a name that would read as a URL is written as the path it is
        assert_eq!(files.link_destination(colon, note), "./Re:%20plan.md");
        assert_eq!(resolved(&files, "./Re:%20gone.md", note), None);
        // a `#` in a name is not where its fragment starts
        assert_eq!(files.resolve_wikilink("C# notes#Intro", note), Some(sharp));
        // a URL no file is named for is the host's
        assert!(matches!(
            files.resolve_link("tel:+15551234", note),
            Some(ResolvedLink::External(_))
        ));
    }

    #[test]
    fn unmatched_wikilinks_have_a_place() {
        let (files, id) = shared_project();
        let note = id("/project/plan.md");
        // where opening the link makes its note, which the link then reaches
        let place = |title: &str, note: Uuid| {
            let (mut parent, names) = files.wikilink_placement(title, note)?;
            let place = format!("{}{}", files.path(parent), names.join("/"));
            let mut files = files.clone();
            for (i, name) in names.iter().enumerate() {
                let file_type =
                    if i + 1 == names.len() { FileType::Document } else { FileType::Folder };
                let made = Uuid::from_u128(100 + i as u128);
                files.push(File { owner: "alice".into(), ..file(made, parent, name, file_type) });
                parent = made;
            }
            assert_eq!(files.resolve_wikilink(title, note), Some(parent), "{title}");
            Some(place)
        };

        assert_eq!(place("Ideas", note).unwrap(), "/project/Ideas.md");
        assert_eq!(place("map.svg", note).unwrap(), "/project/map.svg");
        assert_eq!(place("v1.2 notes", note).unwrap(), "/project/v1.2 notes.md");
        assert_eq!(place("Node.js", note).unwrap(), "/project/Node.js.md");
        assert_eq!(place("Ideas#Open questions", note).unwrap(), "/project/Ideas.md");
        assert_eq!(place("Design/Ideas", note).unwrap(), "/project/design/Ideas.md");
        assert_eq!(place("design/drafts/Ideas", note).unwrap(), "/project/design/drafts/Ideas.md");
        assert_eq!(place("research / Ideas", note).unwrap(), "/project/research/Ideas.md");
        // `/` is the top of the share
        let deep = id("/project/design/plan.md");
        assert_eq!(place("/Ideas", deep).unwrap(), "/project/Ideas.md");
        assert_eq!(place("/Design/Ideas", deep).unwrap(), "/project/design/Ideas.md");
        assert_eq!(place("design/", note), None);
        assert_eq!(place("../Ideas", note), None);
        assert_eq!(place("..", note), None);
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
    fn a_child_is_found_by_name_among_folders_and_documents() {
        let files = fixture::tree("alice", &["/b/", "/a/x.md", "/a.md", "/c.md", "/alice/"]);
        let (cache, root) = (fixture::cache(files.clone()), at(&files, "/"));
        for name in ["a", "b", "a.md", "c.md", "alice", "A.md", "d.md", ""] {
            let found = cache.child_named(root, name).map(|f| f.id);
            assert_eq!(found, files.child_named(root, name).map(|f| f.id), "{name}");
            assert_eq!(found.is_some(), !matches!(name, "A.md" | "d.md" | ""), "{name}");
        }
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
}
