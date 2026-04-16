use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Debug, Formatter};
use std::iter;

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

pub struct FileCache {
    pub root: File,
    pub files: Vec<File>,
    pub shared: Vec<File>,
    pub suggested: Vec<Uuid>,
    pub size_bytes_recursive: HashMap<Uuid, u64>,
    pub last_modified_recursive: HashMap<Uuid, u64>,
    pub last_modified_by_recursive: HashMap<Uuid, String>,
    /// Max last_modified across all files. Used as a cache invalidation key
    /// by the landing page sort cache — changes whenever the file tree changes.
    pub last_modified: u64,
}

impl FileCache {
    /// An empty file cache for contexts where no real files exist (e.g. public site demos).
    pub fn empty() -> Self {
        let root_id = Uuid::new_v4();
        Self {
            root: File {
                id: root_id,
                parent: root_id,
                name: "root".into(),
                file_type: FileType::Folder,
                last_modified: 0,
                last_modified_by: String::new(),
                owner: String::new(),
                shares: vec![],
                size_bytes: 0,
            },
            files: vec![],
            shared: vec![],
            suggested: vec![],
            size_bytes_recursive: Default::default(),
            last_modified_recursive: Default::default(),
            last_modified_by_recursive: Default::default(),
            last_modified: 0,
        }
    }

    #[instrument(level = "debug", skip_all)]
    pub fn new(lb: &Lb) -> LbResult<Self> {
        let root = lb.get_root()?;
        let files = lb.list_metadatas()?;
        let suggested = lb.suggested_docs(Default::default())?;
        let shared = lb.get_pending_share_files()?;

        let mut size_recursive = HashMap::new();
        let mut modified_recursive = HashMap::new();
        let mut modified_by_recursive = HashMap::new();
        for file in files.iter().chain(shared.iter()) {
            let all_ids = files
                .descendents(file.id)
                .iter()
                .chain(shared.descendents(file.id).iter())
                .map(|f| f.id)
                .chain(iter::once(file.id))
                .collect::<Vec<_>>();

            size_recursive.insert(
                file.id,
                all_ids
                    .iter()
                    .filter_map(|id| {
                        files
                            .get_by_id(*id)
                            .or_else(|| shared.get_by_id(*id))
                            .map(|f| f.size_bytes)
                    })
                    .sum::<_>(),
            );

            let most_recent = all_ids
                .iter()
                .filter_map(|id| files.get_by_id(*id).or_else(|| shared.get_by_id(*id)))
                .max_by_key(|f| f.last_modified);

            modified_recursive.insert(file.id, most_recent.map(|f| f.last_modified).unwrap_or(0));
            modified_by_recursive.insert(
                file.id,
                most_recent
                    .map(|f| f.last_modified_by.clone())
                    .unwrap_or_default(),
            );
        }

        let last_modified = files
            .iter()
            .chain(shared.iter())
            .map(|f| f.last_modified)
            .max()
            .unwrap_or(0);

        Ok(Self {
            root,
            files,
            suggested,
            shared,
            size_bytes_recursive: size_recursive,
            last_modified_recursive: modified_recursive,
            last_modified_by_recursive: modified_by_recursive,
            last_modified,
        })
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
        self.files.iter().chain(self.shared.iter())
    }

    /// Returns path segments for a file, each annotated with whether that file
    /// has any shares on it. Segments are in root-to-leaf order. The leading `/`
    /// is included as a separate segment for own-tree files.
    pub fn path_segments(&self, id: Uuid) -> Vec<(String, bool)> {
        let Some(file) = self.get_by_id(id) else {
            return vec![("/".to_string(), false)];
        };
        if file.is_root() {
            return vec![("/".to_string(), false)];
        }

        let mut parts: Vec<(&str, bool)> = Vec::new();
        let mut current = id;
        let mut reached_root = false;
        loop {
            let Some(f) = self.get_by_id(current) else { break };
            if f.is_root() {
                reached_root = true;
                break;
            }
            parts.push((&f.name, !f.shares.is_empty()));
            if self.get_by_id(f.parent).is_none() {
                break; // share boundary
            }
            current = f.parent;
        }
        parts.reverse();

        let mut segments = Vec::new();
        if reached_root {
            segments.push(("/".to_string(), false));
        }
        for (i, (name, shared)) in parts.iter().enumerate() {
            segments.push(((*name).to_string(), *shared));
            let is_last = i + 1 == parts.len();
            if !is_last {
                segments.push(("/".to_string(), false));
            }
        }
        segments
    }

    /// Collects the set of usernames who have access to a file: the owner plus
    /// anyone with a share entry on the file or any of its ancestors.
    pub fn users_with_access(&self, id: Uuid) -> HashSet<&str> {
        let mut users = HashSet::new();
        for ancestor_id in iter::once(id).chain(self.ancestors(id)) {
            let Some(file) = self.get_by_id(ancestor_id) else { break };
            if users.is_empty() {
                users.insert(file.owner.as_str());
            }
            for share in &file.shares {
                users.insert(share.shared_with.as_str());
            }
        }
        users
    }

    /// Returns true if any user with access to `from_id` cannot access `target_id`.
    pub fn link_has_access_gap(&self, from_id: Uuid, target_id: Uuid) -> bool {
        let from_users = self.users_with_access(from_id);
        let target_users = self.users_with_access(target_id);
        from_users.iter().any(|u| !target_users.contains(u))
    }

    /// Returns true if two files are in the same tree. Files in different share
    /// trees or across the user's own tree and a share tree are in different trees.
    /// Two files are in the same tree if walking up ancestors from both reaches the
    /// same root (either the user's root or the same share root).
    pub fn same_tree(&self, a: Uuid, b: Uuid) -> bool {
        self.tree_root(a) == self.tree_root(b)
    }

    /// Walks ancestors to find the tree root: the user's root or the topmost
    /// reachable file (share root, where the parent is not in the cache).
    pub fn tree_root(&self, id: Uuid) -> Uuid {
        let mut current = id;
        loop {
            let Some(file) = self.get_by_id(current) else { return current };
            if file.is_root() {
                return current;
            }
            if self.get_by_id(file.parent).is_none() {
                return current; // share root: parent not in cache
            }
            current = file.parent;
        }
    }

    pub fn last_modified_by_recursive(&self, id: Uuid) -> &str {
        self.last_modified_by_recursive
            .get(&id)
            .map(|s| s.as_str())
            .unwrap_or_else(|| {
                self.get_by_id(id)
                    .map(|f| f.last_modified_by.as_str())
                    .unwrap_or("")
            })
    }
}

impl Debug for FileCache {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCache")
            .field("files.len()", &self.files.len())
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

    /// returns all known parents until we can't find one (share) or we hit root.
    /// Paths rooted in the user's own tree start with `/`. Paths in a share tree
    /// (where the walk stopped at a share boundary) omit the leading `/`.
    fn path(&self, id: Uuid) -> String {
        let Some(file) = self.get_by_id(id) else { return "/".to_string() };
        if file.is_root() {
            return "/".to_string();
        }
        let mut parts = vec![file.name.as_str()];
        let mut current = file.parent;
        let mut reached_root = false;
        loop {
            let Some(f) = self.get_by_id(current) else { break };
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

    /// Resolves a URL from a regular link or image.
    ///
    /// - `lb://uuid` — verified against cache, returned as `File(uuid)`
    /// - external (http/https/mailto/#) — returned as `External(url)`
    /// - relative path — resolved against `from_id`'s folder in the cache
    ///
    /// Only documents resolve to `File`; folders are treated as broken.
    /// Returns None if the URL is an internal path that doesn't resolve.
    fn resolve_link(&self, url: &str, from_id: Uuid) -> Option<ResolvedLink> {
        if let Some(id_str) = url.strip_prefix("lb://") {
            let id = Uuid::parse_str(id_str).ok()?;
            let file = self.get_by_id(id)?;
            if !file.is_document() {
                return None;
            }
            return Some(ResolvedLink::File(id));
        }

        if url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("mailto:")
            || url.starts_with('#')
        {
            return Some(ResolvedLink::External(url.to_string()));
        }

        let parent_path = self.path(from_id);
        let combined = format!("{}/{}", parent_path.trim_end_matches('/'), url);
        let canonical = canonicalize(&combined);
        let decoded = decode(&canonical)
            .map(|c| c.into_owned())
            .unwrap_or(canonical);
        let file = self.by_path(&decoded)?;
        if !file.is_document() {
            return None;
        }
        Some(ResolvedLink::File(file.id))
    }

    /// Resolves a wikilink title to a document UUID.
    ///
    /// - disambiguation paths ("folder/title") resolved via relative path from `from_id`
    /// - bare titles matched case-insensitively against the cache
    /// - on conflict, prefers the file closest to `from_id`
    ///
    /// Only documents match; folders are ignored.
    /// Returns None if no matching document is found.
    fn resolve_wikilink(&self, title: &str, from_id: Uuid) -> Option<Uuid> {
        let parent_path = self.path(from_id);

        if title.contains('/') {
            let with_ext =
                if title.ends_with(".md") { title.to_string() } else { format!("{}.md", title) };
            let combined = format!("{}/{}", parent_path.trim_end_matches('/'), with_ext);
            let canonical = canonicalize(&combined);
            if let Some(file) = self.by_path(&canonical).filter(|f| f.is_document()) {
                return Some(file.id);
            }
        }

        let bare_title = title
            .rsplit('/')
            .next()
            .unwrap_or(title)
            .trim_end_matches(".md");

        let candidates: Vec<_> = self
            .iter_files()
            .filter(|f| f.is_document())
            .filter(|f| {
                f.name
                    .trim_end_matches(".md")
                    .eq_ignore_ascii_case(bare_title)
            })
            .collect();

        match candidates.len() {
            0 => None,
            1 => Some(candidates[0].id),
            _ => candidates
                .iter()
                .min_by_key(|f| {
                    relative_path(&parent_path, &self.path(f.id))
                        .matches("../")
                        .count()
                })
                .map(|f| f.id),
        }
    }

    fn ancestors(&self, id: Uuid) -> Vec<Uuid> {
        let mut ancestors = vec![];
        let mut current = id;
        loop {
            let Some(file) = self.get_by_id(current) else { break };
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

    fn resolve_link(&self, url: &str, from_id: Uuid) -> Option<ResolvedLink> {
        self.as_slice().resolve_link(url, from_id)
    }

    fn resolve_wikilink(&self, title: &str, from_id: Uuid) -> Option<Uuid> {
        self.as_slice().resolve_wikilink(title, from_id)
    }
}

impl FilesExt for FileCache {
    fn root(&self) -> &File {
        self.files.root()
    }

    fn get_by_id(&self, id: Uuid) -> Option<&File> {
        self.files
            .get_by_id(id)
            .or_else(|| self.shared.get_by_id(id))
    }

    fn children(&self, id: Uuid) -> Vec<&File> {
        let mut children: Vec<_> = self
            .all_files()
            .filter(|f| f.parent == id && f.parent != f.id)
            .collect();
        children.sort_by(|a, b| match (a.file_type, b.file_type) {
            (FileType::Folder, FileType::Document) => Ordering::Less,
            (FileType::Document, FileType::Folder) => Ordering::Greater,
            (_, _) => a.name.cmp(&b.name),
        });
        children
    }

    fn iter_files(&self) -> impl Iterator<Item = &File> {
        self.all_files()
    }
}

pub fn relative_path(from: &str, to: &str) -> String {
    if from == to {
        if from.ends_with('/') {
            return "./".to_string();
        } else {
            return ".".to_string();
        }
    }

    let from_parts: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
    let to_parts: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();

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
    for component in path.split('/') {
        match component {
            ".." => {
                parts.pop();
            }
            "" | "." => {}
            _ => parts.push(component),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_rs::model::file_metadata::FileType;

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
}
