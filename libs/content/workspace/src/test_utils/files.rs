//! Hand-built file trees.

use lb_rs::Uuid;
use lb_rs::model::file::{File, Share, ShareMode};
use lb_rs::model::file_metadata::FileType;

use crate::file_cache::{FileCache, FilesExt};

/// `owner`'s tree holding `paths`. A trailing `/` makes a folder, folders
/// along a path are made as needed, and ` @bob,carol` after a path shares
/// what it names with those accounts. Ids count up from 1, the root's.
pub fn tree(owner: &str, paths: &[&str]) -> Vec<File> {
    let file = |id: usize, parent: usize, name: &str, file_type| File {
        id: Uuid::from_u128(id as u128),
        parent: Uuid::from_u128(parent as u128),
        name: name.into(),
        file_type,
        last_modified: 0,
        last_modified_by: String::new(),
        owner: owner.into(),
        shares: vec![],
        size_bytes: 0,
    };
    let mut files = vec![file(1, 1, owner, FileType::Folder)];
    for spec in paths {
        let (path, readers) = spec.split_once(" @").unwrap_or((spec, ""));
        let names: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut parent = files[0].id;
        for (i, name) in names.iter().enumerate() {
            let folder = i + 1 < names.len() || path.ends_with('/');
            let beside = |f: &&File| f.parent == parent && f.id != parent && f.name == *name;
            let existing = files.iter().find(beside);
            parent = match existing {
                Some(f) => f.id,
                None => {
                    let file_type = if folder { FileType::Folder } else { FileType::Document };
                    let id = files.len() + 1;
                    files.push(file(id, parent.as_u128() as usize, name, file_type));
                    Uuid::from_u128(id as u128)
                }
            };
        }
        let named = files.iter_mut().find(|f| f.id == parent).unwrap();
        for reader in readers.split(',').filter(|r| !r.is_empty()) {
            named.shares.push(Share {
                mode: ShareMode::Write,
                shared_by: owner.into(),
                shared_with: reader.into(),
            });
        }
    }
    files
}

pub fn cache(files: Vec<File>) -> FileCache {
    FileCache::from_owned_and_shared(files[0].clone(), files, [])
}

/// The id of the file at `path`.
pub fn at<F: FilesExt + ?Sized>(files: &F, path: &str) -> Uuid {
    files
        .by_path(path)
        .unwrap_or_else(|| panic!("no file at {path}"))
        .id
}
