use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::iter;

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::access_info::UserAccessMode;
use lb_rs::model::account::Account;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file::{File, ShareMode};
use lb_rs::model::file_metadata::FileType;
use std::path::{Component, PathBuf};
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
}

impl FileCache {
    #[instrument(level = "debug", skip_all)]
    pub fn new(lb: &Lb) -> LbResult<Self> {
        let root = lb.get_root()?;
        let files = lb.list_metadatas()?;
        let suggested = lb.suggested_docs(Default::default())?;
        let shared = lb.get_pending_share_files()?;

        let mut size_recursive = HashMap::new();
        for file in &files {
            size_recursive.insert(
                file.id,
                files
                    .descendents(file.id)
                    .iter()
                    .map(|f| f.id)
                    .chain(iter::once(file.id))
                    .map(|id| files.get_by_id(id).unwrap().size_bytes)
                    .sum::<_>(),
            );
        }

        Ok(Self { root, files, suggested, shared, size_bytes_recursive: size_recursive })
    }

    pub fn usage_portion(&self, id: Uuid) -> f32 {
        self.size_bytes_recursive[&id] as f32
            / self.size_bytes_recursive[&self.files.get_by_id(id).unwrap().parent] as f32
    }

    /// returns the uncompressed, recursive size of a file scaled relative to
    /// peers so that the biggest sibling is 1.0
    pub fn usage_portion_scaled(&self, id: Uuid, peers: &[&File]) -> f32 {
        let current_usage = self.size_bytes_recursive[&id];

        let max_sibling_usage = peers
            .iter()
            .map(|peer| self.size_bytes_recursive[&peer.id])
            .chain(std::iter::once(current_usage))
            .max()
            .unwrap_or(1);

        current_usage as f32 / max_sibling_usage as f32
    }

    pub fn last_modified_recursive(&self, id: Uuid) -> u64 {
        self.files
            .descendents(id)
            .iter()
            .map(|f| f.id)
            .chain(iter::once(id))
            .map(|id| self.files.get_by_id(id).unwrap().last_modified)
            .max()
            .unwrap()
    }

    pub fn last_modified_by_recursive(&self, id: Uuid) -> &str {
        let last_modified_id = self
            .files
            .descendents(id)
            .iter()
            .map(|f| f.id)
            .chain(iter::once(id))
            .max_by_key(|id| self.files.get_by_id(*id).unwrap().last_modified)
            .unwrap();
        &self
            .files
            .get_by_id(last_modified_id)
            .unwrap()
            .last_modified_by
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

    /// returns all known parents until we can't find one (share) or we hit root
    fn path(&self, id: Uuid) -> String {
        let Some(file) = self.get_by_id(id) else { return "/".to_string() };
        if file.is_root() {
            return "/".to_string();
        }
        let mut parts = vec![file.name.as_str()];
        let mut current = file.parent;
        loop {
            let Some(f) = self.get_by_id(current) else { break };
            if f.is_root() {
                break;
            }
            parts.push(f.name.as_str());
            current = f.parent;
        }
        parts.reverse();
        let joined = parts.join("/");
        if file.is_folder() { format!("/{joined}/") } else { format!("/{joined}") }
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
        if let Some(us) = self.get_by_id(id) {
            if us.is_root() {
                return ancestors;
            }

            let parent = us.parent;
            ancestors.push(parent);
            ancestors.extend_from_slice(&self.ancestors(parent));
        }
        ancestors
    }

    fn access(&self, id: Uuid, account: &Account) -> UserAccessMode {
        for id in iter::once(id).chain(self.ancestors(id).iter().copied()) {
            let file = self.get_by_id(id).unwrap();
            for share in &file.shares {
                if share.shared_with == account.username {
                    match share.mode {
                        ShareMode::Write => {
                            return UserAccessMode::Write;
                        }
                        ShareMode::Read => {
                            return UserAccessMode::Read;
                        }
                    }
                }
            }
        }
        UserAccessMode::Owner
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
        self.files.get_by_id(id)
    }

    fn children(&self, id: Uuid) -> Vec<&File> {
        self.files.children(id)
    }

    fn iter_files(&self) -> impl Iterator<Item = &File> {
        self.files.iter_files()
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

    let from_path = PathBuf::from(from);
    let to_path = PathBuf::from(to);

    let mut num_common_ancestors = 0;
    for (from_component, to_component) in from_path.components().zip(to_path.components()) {
        if from_component != to_component {
            break;
        }
        num_common_ancestors += 1;
    }

    let mut result = "../".repeat(from_path.components().count() - num_common_ancestors);
    for to_component in to_path.components().skip(num_common_ancestors) {
        result.push_str(to_component.as_os_str().to_str().unwrap());
        result.push('/');
    }
    if !to.ends_with('/') {
        result.pop();
    }
    result
}

pub fn canonicalize(path: &str) -> String {
    let path = PathBuf::from(path);
    let mut result = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(component) => {
                result.push(component);
            }
            Component::ParentDir => {
                result.pop();
            }
            _ => {}
        }
    }

    result.to_string_lossy().to_string()
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
