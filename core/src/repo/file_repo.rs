use crate::model::repo::RepoSource;
use crate::model::repo::RepoState;
use crate::model::state::Config;
use crate::repo::digest_repo;
use crate::repo::document_repo;
use crate::repo::metadata_repo;
use crate::repo::root_repo;
use crate::utils::metadata_repo_state_vec_to_map;
use crate::utils::metadata_vec_to_map;
use crate::utils::slices_equal;
use crate::CoreError;
use lockbook_models::crypto::EncryptedDocument;
use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::file_metadata::FileMetadataDiff;
use lockbook_models::file_metadata::FileType;
use std::collections::HashMap;
use std::collections::HashSet;
use uuid::Uuid;

fn get_metadata_include_deleted(
    config: &Config,
    id: Uuid,
) -> Result<(FileMetadata, RepoState), CoreError> {
    maybe_get_metadata_include_deleted(config, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

fn maybe_get_metadata_include_deleted(
    config: &Config,
    id: Uuid,
) -> Result<Option<(FileMetadata, RepoState)>, CoreError> {
    let maybe_local = metadata_repo::maybe_get(config, RepoSource::Local, id)?;
    let maybe_remote = metadata_repo::maybe_get(config, RepoSource::Remote, id)?;
    let (target, state) = match (maybe_local, maybe_remote) {
        (None, None) => {
            return Ok(None);
        }
        (Some(local), None) => (local, RepoState::New), // new files are only stored in the local repo
        (None, Some(remote)) => (remote, RepoState::Unmodified), // unmodified files are only stored in the remote repo
        (Some(local), Some(remote)) => (local, RepoState::Modifed), // modified files are stored in both repos
    };
    Ok(Some((target, state)))
}

pub fn get_with_ancestors(
    config: &Config,
    id: Uuid,
) -> Result<HashMap<Uuid, (FileMetadata, RepoState)>, CoreError> {
    maybe_get_with_ancestors(config, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_with_ancestors(
    config: &Config,
    id: Uuid,
) -> Result<Option<HashMap<Uuid, (FileMetadata, RepoState)>>, CoreError> {
    let result = vec![get_metadata(config, id)?];
    append_ancestors(config, &mut result)?;
    // file_repo functions do not return deleted files (including files with deleted ancestors) unless their name ends with _include_deleted
    if result.iter().any(|f| f.0.deleted) {
        Ok(None)
    } else {
        Ok(Some(metadata_repo_state_vec_to_map(result)))
    }
}

fn append_ancestors(
    config: &Config,
    result: &mut Vec<(FileMetadata, RepoState)>,
) -> Result<(), CoreError> {
    let target = result
        .last()
        .ok_or_else(|| CoreError::Unexpected(String::from("append ancestors with no target")))?
        .0;
    let original = result
        .first()
        .ok_or_else(|| CoreError::Unexpected(String::from("append ancestors with no target")))?
        .0;
    if target.parent != target.id {
        if target.parent == original.id {
            return Err(CoreError::FolderMovedIntoSelf);
        }
        result.push(get_metadata_include_deleted(config, target.parent)?);
        append_ancestors(config, result)?;
    }
    Ok(())
}

pub fn get_metadata(config: &Config, id: Uuid) -> Result<(FileMetadata, RepoState), CoreError> {
    maybe_get_metadata(config, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_metadata(
    config: &Config,
    id: Uuid,
) -> Result<Option<(FileMetadata, RepoState)>, CoreError> {
    // getting ancestors ensures we do not return a file with a deleted ancestor
    let maybe_ancestors = maybe_get_with_ancestors(config, id)?;
    Ok(match maybe_ancestors {
        Some(ancestors) => {
            let result = ancestors
                .get(&id)
                .ok_or(CoreError::Unexpected(String::from(
                    "ancestors of file did not include file",
                )))?;
            Some((result.0.clone(), result.1.clone()))
        }
        None => None,
    })
}

// managing the results when you get multiple files is annoying; this struct and it's impl can help
pub struct GetAllMetadataResult {
    pub unmodified: Vec<FileMetadata>,
    pub modified: Vec<FileMetadata>,
    pub new: Vec<FileMetadata>,
}

impl GetAllMetadataResult {
    pub fn union(self: Self) -> Vec<FileMetadata> {
        self.union_new_and_modified()
            .into_iter()
            .chain(self.unmodified.into_iter())
            .collect()
    }

    pub fn union_new_and_modified(self: Self) -> Vec<FileMetadata> {
        self.new
            .into_iter()
            .chain(self.modified.into_iter())
            .collect()
    }

    pub fn union_with_state(self: Self) -> Vec<(FileMetadata, RepoState)> {
        let unmodified_with_state = self
            .unmodified
            .into_iter()
            .map(|u| (u, RepoState::Unmodified));
        let modified_with_state = self.modified.into_iter().map(|u| (u, RepoState::Modifed));
        let new_with_state = self.new.into_iter().map(|u| (u, RepoState::New));
        unmodified_with_state
            .chain(modified_with_state)
            .chain(new_with_state)
            .collect()
    }

    pub fn from_union_with_state(files: Vec<(FileMetadata, RepoState)>) -> Self {
        let mut result = GetAllMetadataResult {
            unmodified: Vec::new(),
            modified: Vec::new(),
            new: Vec::new(),
        };
        for (file, state) in files {
            match state {
                RepoState::Unmodified => {
                    result.unmodified.push(file);
                }
                RepoState::Modifed => {
                    result.modified.push(file);
                }
                RepoState::New => {
                    result.new.push(file);
                }
            }
        }
        result
    }
}

fn get_all_metadata_include_deleted(config: &Config) -> Result<GetAllMetadataResult, CoreError> {
    let local = metadata_repo::get_all(config, RepoSource::Local)?;
    let remote = metadata_repo::get_all(config, RepoSource::Remote)?;
    let local_map = metadata_vec_to_map(local);
    let remote_map = metadata_vec_to_map(remote);
    let distinct_ids = local
        .iter()
        .map(|f| f.id)
        .chain(remote.iter().map(|f| f.id))
        .collect::<HashSet<Uuid>>();
    let mut result = GetAllMetadataResult {
        unmodified: Vec::new(),
        modified: Vec::new(),
        new: Vec::new(),
    };
    for id in distinct_ids {
        match (local_map.get(&id), remote_map.get(&id)) {
            (None, None) => {
                return Err(CoreError::Unexpected(String::from(
                    "neither of two maps contained a key that came from the union of their keys",
                )))
            }
            (Some(local), None) => {
                result.new.push(local.clone());
            }
            (None, Some(remote)) => {
                result.unmodified.push(remote.clone());
            }
            (Some(local), Some(_)) => {
                if !local.deleted {
                    result.new.push(local.clone());
                }
            }
        }
    }
    Ok(result)
}

// note: includes target file (hence 'with' in the name)
pub fn get_with_descendants(
    config: &Config,
    id: Uuid,
) -> Result<Vec<(FileMetadata, RepoState)>, CoreError> {
    let all = get_all_metadata_include_deleted(config)?
        .union_with_state()
        .into_iter()
        .filter(|(f, _)| !f.deleted)
        .collect();
    let result = vec![get_metadata(config, id)?];
    append_descendants_recursive(config, &all, id, &mut result)?;
    Ok(result)
}

fn append_descendants_recursive(
    config: &Config,
    all: &Vec<(FileMetadata, RepoState)>,
    id: Uuid,
    result: &mut Vec<(FileMetadata, RepoState)>,
) -> Result<(), CoreError> {
    for child in all.iter().filter(|f| f.0.parent == id) {
        result.push(child.clone());
        append_descendants_recursive(config, all, child.0.id, result)?;
    }
    Ok(())
}

pub fn get_all_metadata(config: &Config) -> Result<GetAllMetadataResult, CoreError> {
    Ok(GetAllMetadataResult::from_union_with_state(
        get_with_descendants(config, root_repo::get(config)?.id)?,
    ))
}

// note: does not include target file
pub fn get_children(config: &Config, id: Uuid) -> Result<Vec<FileMetadata>, CoreError> {
    Ok(get_all_metadata(config)?
        .union()
        .into_iter()
        .filter(|file| file.parent == id && file.parent != file.id)
        .collect::<Vec<FileMetadata>>())
}

pub fn get_document(
    config: &Config,
    id: Uuid,
) -> Result<(EncryptedDocument, RepoState), CoreError> {
    maybe_get_document(config, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_document(
    config: &Config,
    id: Uuid,
) -> Result<Option<(EncryptedDocument, RepoState)>, CoreError> {
    if maybe_get_metadata(config, id)?.is_none() {
        Ok(None)
    } else {
        let maybe_local = document_repo::maybe_get(config, RepoSource::Local, id)?;
        let maybe_remote = document_repo::maybe_get(config, RepoSource::Remote, id)?;
        match (maybe_local, maybe_remote) {
            (None, None) => Ok(None),
            (Some(local), None) => Ok(Some((local, RepoState::New))),
            (None, Some(remote)) => Ok(Some((remote, RepoState::Unmodified))),
            (Some(local), Some(remote)) => Ok(Some((local, RepoState::Modifed))),
        }
    }
}

pub fn get_all_metadata_changes(config: &Config) -> Result<Vec<FileMetadataDiff>, CoreError> {
    let local = metadata_repo::get_all(config, RepoSource::Local)?;
    let remote = metadata_repo::get_all(config, RepoSource::Remote)?;

    let new = local
        .iter()
        .filter(|l| !remote.iter().any(|r| r.id == l.id))
        .map(|l| FileMetadataDiff::new(l));
    let changed = local
        .iter()
        .filter_map(|l| match remote.iter().find(|r| r.id == l.id) {
            Some(r) => Some((l, r)),
            None => None,
        })
        .map(|(l, r)| FileMetadataDiff::new_diff(r.parent, &r.name, l));

    Ok(new.chain(changed).collect())
}

pub fn get_all_with_document_changes(config: &Config) -> Result<Vec<Uuid>, CoreError> {
    Ok(get_all_metadata(config)?
        .union()
        .into_iter()
        .map(|f| {
            Ok((
                f.id,
                digest_repo::maybe_get(config, RepoSource::Local, f.id)?.is_some(),
            ))
        })
        .collect::<Result<Vec<(Uuid, bool)>, CoreError>>()?
        .into_iter()
        .filter_map(|(id, has_local_change)| match has_local_change {
            true => Some(id),
            false => None,
        })
        .collect())
}

pub fn insert_metadata(
    config: &Config,
    source: RepoSource,
    file: &FileMetadata,
) -> Result<(), CoreError> {
    metadata_repo::insert(config, source, &file)?;

    // remove local if local == remote
    if let Some(opposite) = metadata_repo::maybe_get(config, source.opposite(), file.id)? {
        if slices_equal(&opposite.name.hmac, &file.name.hmac)
            && opposite.parent == file.parent
            && opposite.deleted == file.deleted
        {
            metadata_repo::delete(config, RepoSource::Local, file.id)?;
        }
    }

    // delete documents from disk if their metadata is set to deleted
    if file.deleted {
        if let Some((document, _)) = maybe_get_document(config, file.id)? {
            delete_content(config, file.id)?;
        }
    }

    Ok(())
}

pub fn insert_document(
    config: &Config,
    source: RepoSource,
    id: Uuid,
    document: EncryptedDocument,
    digest: &[u8],
) -> Result<(), CoreError> {
    document_repo::insert(config, RepoSource::Local, id, &document)?;
    digest_repo::insert(config, RepoSource::Local, id, digest)?;

    // remove local if local == remote
    if let Some(opposite) = digest_repo::maybe_get(config, source.opposite(), id)? {
        if slices_equal(&opposite, digest) {
            document_repo::delete(config, RepoSource::Local, id)?;
            digest_repo::delete(config, RepoSource::Local, id)?;
        }
    }

    Ok(())
}

// apply a set of operations then call this (e.g. during sync), because otherwise you'll prune e.g. a file that was moved out of a folder that was deleted
pub fn prune_deleted(config: &Config) -> Result<(), CoreError> {
    let all_include_deleted = get_all_metadata_include_deleted(config)?.union();
    let all_exclude_deleted = get_all_metadata(config)?.union();
    for file in all_include_deleted {
        if !all_exclude_deleted.iter().any(|f| f.id == file.id) {
            delete_metadata(config, file.id)?;
            if file.file_type == FileType::Document {
                delete_content(config, file.id)?;
            }
        }
    }
    Ok(())
}

fn delete_metadata(config: &Config, id: Uuid) -> Result<(), CoreError> {
    metadata_repo::delete(config, RepoSource::Local, id)?;
    metadata_repo::delete(config, RepoSource::Remote, id)
}

fn delete_content(config: &Config, id: Uuid) -> Result<(), CoreError> {
    document_repo::delete(config, RepoSource::Local, id)?;
    document_repo::delete(config, RepoSource::Remote, id)?;
    digest_repo::delete(config, RepoSource::Local, id)?;
    digest_repo::delete(config, RepoSource::Remote, id)
}
