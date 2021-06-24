use crate::model::repo::RepoSource;
use crate::model::repo::RepoState;
use crate::model::state::Config;
use crate::repo::digest_repo;
use crate::repo::document_repo;
use crate::repo::metadata_repo;
use crate::utils::metadata_vec_to_map;
use crate::utils::slices_equal;
use crate::CoreError;
use lockbook_models::crypto::EncryptedDocument;
use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::file_metadata::FileMetadataDiff;
use std::collections::HashMap;
use std::collections::HashSet;
use uuid::Uuid;

pub fn maybe_get_metadata(
    config: &Config,
    id: Uuid,
) -> Result<Option<(FileMetadata, RepoState)>, CoreError> {
    let maybe_local = metadata_repo::maybe_get(config, RepoSource::Local, id)?;
    let maybe_remote = metadata_repo::maybe_get(config, RepoSource::Remote, id)?;
    match (maybe_local, maybe_remote) {
        (None, None) => Ok(None),
        (Some(local), None) => Ok(Some((local, RepoState::New))),
        (None, Some(remote)) => Ok(Some((remote, RepoState::Unmodified))),
        (Some(local), Some(remote)) => {
            if local.deleted {
                Ok(None)
            } else {
                Ok(Some((local, RepoState::Modifed)))
            }
        }
    }
}

pub fn get_metadata(config: &Config, id: Uuid) -> Result<(FileMetadata, RepoState), CoreError> {
    maybe_get_metadata(config, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_document(
    config: &Config,
    id: Uuid,
) -> Result<Option<(EncryptedDocument, RepoState)>, CoreError> {
    if maybe_get_metadata(config, id)?.is_none() {
        return Ok(None);
    }
    let maybe_local = document_repo::maybe_get(config, RepoSource::Local, id)?;
    let maybe_remote = document_repo::maybe_get(config, RepoSource::Remote, id)?;
    match (maybe_local, maybe_remote) {
        (None, None) => Ok(None),
        (Some(local), None) => Ok(Some((local, RepoState::New))),
        (None, Some(remote)) => Ok(Some((remote, RepoState::Unmodified))),
        (Some(local), Some(remote)) => Ok(Some((local, RepoState::Modifed))),
    }
}

pub fn get_document(
    config: &Config,
    id: Uuid,
) -> Result<(EncryptedDocument, RepoState), CoreError> {
    maybe_get_document(config, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub struct GetAllMetadataResult {
    pub unmodified: Vec<FileMetadata>,
    pub modified: Vec<FileMetadata>,
    pub new: Vec<FileMetadata>,
}

impl GetAllMetadataResult {
    pub fn union(self: Self) -> Vec<FileMetadata> {
        self.new
            .into_iter()
            .chain(self.modified.into_iter().chain(self.unmodified.into_iter()))
            .collect()
    }

    pub fn union_new_and_modified(self: Self) -> Vec<FileMetadata> {
        self.new
            .into_iter()
            .chain(self.modified.into_iter())
            .collect()
    }
}

pub fn get_all_metadata(config: &Config) -> Result<GetAllMetadataResult, CoreError> {
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

pub fn get_with_ancestors(
    config: &Config,
    id: Uuid,
) -> Result<HashMap<Uuid, FileMetadata>, CoreError> {
    let result = vec![get_metadata(config, id)?.0];
    append_ancestors(config, &mut result)?;
    Ok(metadata_vec_to_map(result))
}

fn append_ancestors(config: &Config, result: &mut Vec<FileMetadata>) -> Result<(), CoreError> {
    let target = result
        .last()
        .ok_or_else(|| CoreError::Unexpected(String::from("append ancestors with no target")))?;
    let original = result
        .first()
        .ok_or_else(|| CoreError::Unexpected(String::from("append ancestors with no target")))?;
    if target.parent != target.id {
        if target.parent == original.id {
            return Err(CoreError::FolderMovedIntoSelf);
        }
        result.push(get_metadata(config, target.parent)?.0);
        append_ancestors(config, result)?;
    }
    Ok(())
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

pub fn delete(config: &Config, source: RepoSource, id: Uuid) -> Result<(), CoreError> {
    // forget children, if any
    let all = get_all_metadata(config)?.union();
    delete_children_recursive(config, all, id)?;

    let (mut target, state) = get_metadata(config, id)?;
    match (source, state) {
        (RepoSource::Local, RepoState::Modifed) | (RepoSource::Local, RepoState::Unmodified) => {
            // locally delete a modifed file that exists on remote: mark as deleted (will be pushed during sync)
            target.deleted = true;
            insert_metadata(config, RepoSource::Local, &target)?;
            delete_content(config, id)
        }
        _ => {
            // locally delete a new file or remote delete a file: just forget about it
            delete_metadata(config, id)?;
            delete_content(config, id)
        }
    }
}

fn delete_children_recursive(
    config: &Config,
    all: Vec<FileMetadata>,
    id: Uuid,
) -> Result<(), CoreError> {
    for child in all.iter().filter(|f| f.parent == id) {
        delete_children_recursive(config, all, child.id)?;
        delete_content(config, child.id)?;
        delete_metadata(config, child.id)?;
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

pub fn get_children_non_recursive(
    config: &Config,
    id: Uuid,
) -> Result<Vec<FileMetadata>, CoreError> {
    Ok(get_all_metadata(config)?
        .union()
        .into_iter()
        .filter(|file| file.parent == id && file.parent != file.id)
        .collect::<Vec<FileMetadata>>())
}

// todo: needed?
pub fn get_all_metadata_changes(config: &Config) -> Result<Vec<FileMetadataDiff>, CoreError> {
    let remote_metadata = metadata_repo::get_all(config, RepoSource::Remote)?;
    let local_metadata = metadata_repo::get_all(config, RepoSource::Remote)?;

    let new = local_metadata
        .iter()
        .filter(|l| !remote_metadata.iter().any(|r| r.id == l.id))
        .map(|l| FileMetadataDiff::new(l));
    let changed = local_metadata
        .iter()
        .filter_map(|l| match remote_metadata.iter().find(|r| r.id == l.id) {
            Some(r) => Some((l, r)),
            None => None,
        })
        .map(|(l, r)| FileMetadataDiff::new_diff(r.parent, &r.name, l));

    Ok(new.chain(changed).collect())
}
