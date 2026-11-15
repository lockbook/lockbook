use std::collections::HashMap;

use lb::Uuid;
use lb::model::api::METADATA_FEE;
use lb::model::file::File;
use lb::model::file_metadata::FileType;
use lb::service::usage::UsageEntry;
use workspace_rs::show::DocType;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub synced: u64,
    pub local: u64,
}

impl Size {
    pub fn delta(self) -> i64 {
        self.local as i64 - self.synced as i64
    }

    fn add(&mut self, other: Size) {
        self.synced += other.synced;
        self.local += other.local;
    }

    fn of(entry: &UsageEntry) -> Self {
        Self { synced: entry.synced, local: entry.local }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Notes,
    Drawings,
    Images,
    Pdfs,
    Text,
    Other,
    Folders,
}

impl Kind {
    pub fn of(name: &str, file_type: FileType) -> Self {
        if file_type != FileType::Document {
            return Self::Folders;
        }
        match DocType::from_name(name) {
            DocType::Markdown => Self::Notes,
            DocType::SVG => Self::Drawings,
            DocType::Image | DocType::ImageUnsupported => Self::Images,
            DocType::PDF => Self::Pdfs,
            DocType::PlainText | DocType::Code => Self::Text,
            DocType::Chat | DocType::Unknown => Self::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Notes => "Notes",
            Self::Drawings => "Drawings",
            Self::Images => "Images",
            Self::Pdfs => "PDFs",
            Self::Text => "Text & code",
            Self::Other => "Other files",
            Self::Folders => "Folders & links",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindRow {
    pub kind: Kind,
    pub count: usize,
    pub size: Size,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: Uuid,
    pub name: String,
    pub is_folder: bool,
    pub count: usize,
    pub size: Size,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    New,
    Grew,
    Shrank,
    MovedOut,
    Deleted,
}

impl Change {
    pub fn label(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::Grew | Self::Shrank => "Edited",
            Self::MovedOut => "Moved to someone else's folder",
            Self::Deleted => "Deleted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeRow {
    pub id: Option<Uuid>,
    pub name: String,
    pub is_folder: bool,
    pub change: Change,
    pub count: usize,
    pub size: Size,
}

#[derive(Clone, Debug, Default)]
pub struct StorageReport {
    pub root: Uuid,
    files: HashMap<Uuid, (String, Uuid, bool)>,
    children: HashMap<Uuid, Vec<Uuid>>,
    sizes: HashMap<Uuid, Size>,
    totals: HashMap<Uuid, (Size, usize)>,
    pub kinds: Vec<KindRow>,
    pub largest: Vec<Row>,
    pub changes: Vec<ChangeRow>,
}

const LARGEST: usize = 12;

impl StorageReport {
    pub fn new<'a>(
        root: Uuid, files: impl IntoIterator<Item = &'a File>, entries: &[UsageEntry],
    ) -> Self {
        let mut report = Self { root, ..Default::default() };
        let mut types = HashMap::new();
        for file in files {
            report
                .files
                .insert(file.id, (file.name.clone(), file.parent, file.is_folder()));
            types.insert(file.id, Kind::of(&file.name, file.file_type));
            if file.id != root {
                report
                    .children
                    .entry(file.parent)
                    .or_default()
                    .push(file.id);
            }
        }

        let mut kinds: HashMap<Kind, KindRow> = HashMap::new();
        let mut deleted = ChangeRow {
            id: None,
            name: String::new(),
            is_folder: false,
            change: Change::Deleted,
            count: 0,
            size: Size::default(),
        };

        for entry in entries {
            let size = Size::of(entry);
            report.sizes.insert(entry.id, size);

            let visible = report.files.contains_key(&entry.id);
            if visible || entry.local > 0 {
                let kind = types.get(&entry.id).copied().unwrap_or(Kind::Folders);
                let row =
                    kinds
                        .entry(kind)
                        .or_insert(KindRow { kind, count: 0, size: Size::default() });
                row.count += 1;
                row.size.add(size);
                report.roll_up(entry.id, size, visible);
            } else {
                deleted.count += 1;
                deleted.size.add(size);
                report.roll_up(entry.id, size, false);
            }

            if entry.synced != entry.local && visible {
                let (name, _, is_folder) = report.files[&entry.id].clone();
                let change = match (entry.synced, entry.local) {
                    (synced, _) if synced == 0 || (!is_folder && synced <= METADATA_FEE) => {
                        Change::New
                    }
                    (_, 0) => Change::MovedOut,
                    (synced, local) if local > synced => Change::Grew,
                    _ => Change::Shrank,
                };
                report.changes.push(ChangeRow {
                    id: Some(entry.id),
                    name,
                    is_folder,
                    change,
                    count: 1,
                    size,
                });
            }
        }

        if deleted.count > 0 {
            deleted.name = match deleted.count {
                1 => "1 deleted file".into(),
                n => format!("{n} deleted files"),
            };
            report.changes.push(deleted);
        }
        report
            .changes
            .sort_by_key(|row| std::cmp::Reverse(row.size.delta().unsigned_abs()));

        report.kinds = kinds
            .into_values()
            .filter(|row| row.size.local > 0)
            .collect();
        report
            .kinds
            .sort_by_key(|row| std::cmp::Reverse(row.size.local));

        let mut largest: Vec<Row> = report
            .files
            .iter()
            .filter(|(_, (_, _, is_folder))| !is_folder)
            .filter_map(|(&id, _)| report.row(id))
            .filter(|row| row.size.local > 0)
            .collect();
        largest.sort_by_key(|row| std::cmp::Reverse(row.size.local));
        largest.truncate(LARGEST);
        report.largest = largest;

        report
    }

    fn roll_up(&mut self, id: Uuid, size: Size, visible: bool) {
        let mut at = if visible { id } else { self.root };
        let mut seen = 0;
        loop {
            let total = self.totals.entry(at).or_default();
            total.0.add(size);
            total.1 += 1;
            if at == self.root || seen > self.files.len() {
                break;
            }
            seen += 1;
            at = match self.files.get(&at) {
                Some((_, parent, _)) if self.files.contains_key(parent) => *parent,
                _ => self.root,
            };
        }
    }

    #[cfg(test)]
    pub fn total(&self) -> Size {
        self.totals
            .get(&self.root)
            .map(|(size, _)| *size)
            .unwrap_or_default()
    }

    pub fn row(&self, id: Uuid) -> Option<Row> {
        let (name, _, is_folder) = self.files.get(&id)?.clone();
        let (size, count) = if is_folder {
            self.totals.get(&id).copied()?
        } else {
            (self.sizes.get(&id).copied()?, 1)
        };
        Some(Row { id, name, is_folder, count, size })
    }

    pub fn children(&self, folder: Uuid) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .children
            .get(&folder)
            .into_iter()
            .flatten()
            .filter_map(|&id| self.row(id))
            .filter(|row| row.size.local > 0 || row.size.synced > 0)
            .collect();
        rows.sort_by(|a, b| {
            b.size
                .local
                .cmp(&a.size.local)
                .then_with(|| a.name.cmp(&b.name))
        });
        rows
    }

    pub fn trail(&self, folder: Uuid) -> Vec<(Uuid, String)> {
        let mut trail = vec![];
        let mut at = folder;
        while let Some((name, parent, _)) = self.files.get(&at) {
            trail.push((at, name.clone()));
            if at == self.root || trail.len() > self.files.len() {
                break;
            }
            at = *parent;
        }
        trail.reverse();
        trail
    }

    pub fn parent_path(&self, id: Uuid) -> String {
        let Some((_, parent, _)) = self.files.get(&id) else { return String::new() };
        let trail = self.trail(*parent);
        if trail.len() <= 1 {
            return "/".into();
        }
        trail
            .iter()
            .skip(1)
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>()
            .join("/")
    }

    pub fn contains_folder(&self, id: Uuid) -> bool {
        self.files
            .get(&id)
            .is_some_and(|(_, _, is_folder)| *is_folder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: Uuid, parent: Uuid, name: &str, file_type: FileType) -> File {
        File {
            id,
            parent,
            name: name.into(),
            file_type,
            last_modified: 0,
            last_modified_by: String::new(),
            owner: String::new(),
            shares: vec![],
            size_bytes: 0,
        }
    }

    fn entry(id: Uuid, synced: u64, local: u64) -> UsageEntry {
        UsageEntry { id, synced, local }
    }

    struct Tree {
        root: Uuid,
        projects: Uuid,
        note: Uuid,
        drawing: Uuid,
        photo: Uuid,
        files: Vec<File>,
    }

    fn tree() -> Tree {
        let root = Uuid::new_v4();
        let projects = Uuid::new_v4();
        let note = Uuid::new_v4();
        let drawing = Uuid::new_v4();
        let photo = Uuid::new_v4();
        let files = vec![
            file(root, root, "me", FileType::Folder),
            file(projects, root, "projects", FileType::Folder),
            file(note, projects, "plan.md", FileType::Document),
            file(drawing, projects, "sketch.svg", FileType::Document),
            file(photo, root, "photo.png", FileType::Document),
        ];
        Tree { root, projects, note, drawing, photo, files }
    }

    #[test]
    fn folders_roll_up_their_descendants() {
        let t = tree();
        let entries = [
            entry(t.root, 1000, 1000),
            entry(t.projects, 1000, 1000),
            entry(t.note, 5000, 9000),
            entry(t.drawing, 3000, 3000),
            entry(t.photo, 20000, 20000),
        ];
        let report = StorageReport::new(t.root, &t.files, &entries);

        assert_eq!(report.total(), Size { synced: 30000, local: 34000 });
        let top = report.children(t.root);
        assert_eq!(top[0].id, t.photo);
        assert_eq!(top[1].id, t.projects);
        assert_eq!(top[1].size, Size { synced: 9000, local: 13000 });
        assert_eq!(top[1].count, 3);
        assert_eq!(report.trail(t.note).len(), 3);
        assert_eq!(report.parent_path(t.note), "projects");
        assert_eq!(report.parent_path(t.photo), "/");
    }

    #[test]
    fn kinds_group_documents_by_type() {
        let t = tree();
        let entries = [
            entry(t.root, 1000, 1000),
            entry(t.projects, 1000, 1000),
            entry(t.note, 5000, 5000),
            entry(t.drawing, 3000, 3000),
            entry(t.photo, 20000, 20000),
        ];
        let report = StorageReport::new(t.root, &t.files, &entries);
        let kinds: Vec<Kind> = report.kinds.iter().map(|row| row.kind).collect();

        assert_eq!(kinds, vec![Kind::Images, Kind::Notes, Kind::Drawings, Kind::Folders]);
        assert_eq!(report.kinds[3].count, 2);
        assert_eq!(report.largest[0].id, t.photo);
        assert_eq!(report.largest.len(), 3);
    }

    #[test]
    fn changes_name_what_will_sync() {
        let t = tree();
        let gone = Uuid::new_v4();
        let link = Uuid::new_v4();
        let entries = [
            entry(t.root, 1000, 1000),
            entry(t.projects, 1000, 1000),
            entry(t.note, 1000, 4000),
            entry(t.drawing, 3000, 1000),
            entry(t.photo, 20000, 0),
            entry(gone, 7000, 0),
            entry(link, 1000, 1000),
        ];
        let report = StorageReport::new(t.root, &t.files, &entries);

        let changes: Vec<Change> = report.changes.iter().map(|row| row.change).collect();
        assert_eq!(changes, vec![Change::MovedOut, Change::Deleted, Change::New, Change::Shrank]);
        assert_eq!(report.changes[1].count, 1);
        assert_eq!(report.total().local, 1000 + 1000 + 4000 + 1000 + 1000);
        assert_eq!(report.total().synced, 1000 + 1000 + 1000 + 3000 + 20000 + 7000 + 1000);
    }
}
