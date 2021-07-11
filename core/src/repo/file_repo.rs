use crate::model::repo::RepoSource;
use crate::model::repo::RepoState;
use crate::model::state::Config;
use crate::repo::digest_repo;
use crate::repo::document_repo;
use crate::repo::metadata_repo;
use crate::repo::root_repo;
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
    source: RepoSource,
    id: Uuid,
) -> Result<FileMetadata, CoreError> {
    maybe_get_metadata_include_deleted(config, source, id)
        .and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

fn maybe_get_metadata_include_deleted(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<FileMetadata>, CoreError> {
    let maybe_local = metadata_repo::maybe_get(config, RepoSource::Local, id)?;
    let maybe_remote = metadata_repo::maybe_get(config, RepoSource::Remote, id)?;
    Ok(RepoState::from_local_and_remote(maybe_local, maybe_remote).and_then(|s| s.source(source)))
}

pub fn get_with_ancestors(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<HashMap<Uuid, FileMetadata>, CoreError> {
    maybe_get_with_ancestors(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_with_ancestors(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<HashMap<Uuid, FileMetadata>>, CoreError> {
    let mut result = vec![get_metadata(config, source, id)?];
    append_ancestors(config, source, &mut result)?;
    // file_repo functions do not return deleted files (including files with deleted ancestors) unless their name ends with _include_deleted
    if result.iter().any(|f| f.deleted) {
        Ok(None)
    } else {
        Ok(Some(metadata_vec_to_map(result)))
    }
}

fn append_ancestors(
    config: &Config,
    source: RepoSource,
    result: &mut Vec<FileMetadata>,
) -> Result<(), CoreError> {
    let target = result
        .last()
        .ok_or_else(|| CoreError::Unexpected(String::from("append ancestors with no target")))?
        .clone();
    let original = result
        .first()
        .ok_or_else(|| CoreError::Unexpected(String::from("append ancestors with no target")))?
        .clone();
    if target.parent != target.id {
        if target.parent == original.id {
            return Err(CoreError::FolderMovedIntoSelf);
        }
        result.push(get_metadata_include_deleted(config, source, target.parent)?);
        append_ancestors(config, source, result)?;
    }
    Ok(())
}

pub fn get_metadata(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<FileMetadata, CoreError> {
    maybe_get_metadata(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_metadata(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<FileMetadata>, CoreError> {
    // getting ancestors ensures we do not return a file with a deleted ancestor
    let maybe_ancestors = maybe_get_with_ancestors(config, source, id)?;
    Ok(match maybe_ancestors {
        Some(ancestors) => {
            let result = ancestors
                .get(&id)
                .ok_or(CoreError::Unexpected(String::from(
                    "ancestors of file did not include file",
                )))?;
            Some(result.clone())
        }
        None => None,
    })
}

fn get_all_metadata_include_deleted(
    config: &Config,
    source: RepoSource,
) -> Result<Vec<FileMetadata>, CoreError> {
    let local = metadata_repo::get_all(config, RepoSource::Local)?;
    let remote = metadata_repo::get_all(config, RepoSource::Remote)?;
    let distinct_ids = local
        .iter()
        .map(|f| f.id)
        .chain(remote.iter().map(|f| f.id))
        .collect::<HashSet<Uuid>>();
    let local_map = metadata_vec_to_map(local);
    let remote_map = metadata_vec_to_map(remote);
    let mut result = Vec::new();
    for id in distinct_ids {
        let maybe_local = local_map.get(&id);
        let maybe_remote = remote_map.get(&id);
        if let Some(sourced) = RepoState::from_local_and_remote(maybe_local, maybe_remote)
            .and_then(|s| s.source(source))
        {
            result.push(sourced.clone());
        }
    }
    Ok(result)
}

// note: includes target file (hence 'with' in the name)
pub fn get_with_descendants(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Vec<FileMetadata>, CoreError> {
    let all = get_all_metadata_include_deleted(config, source)?
        .into_iter()
        .filter(|f| !f.deleted)
        .collect();
    let mut result = vec![get_metadata(config, source, id)?];
    append_descendants_recursive(config, source, &all, id, &mut result)?;
    Ok(result)
}

fn append_descendants_recursive(
    config: &Config,
    source: RepoSource,
    all: &Vec<FileMetadata>,
    id: Uuid,
    result: &mut Vec<FileMetadata>,
) -> Result<(), CoreError> {
    for child in all.iter().filter(|f| f.parent == id) {
        result.push(child.clone());
        append_descendants_recursive(config, source, all, child.id, result)?;
    }
    Ok(())
}

pub fn get_root(config: &Config, source: RepoSource) -> Result<FileMetadata, CoreError> {
    maybe_get_root(config, source).and_then(|f| f.ok_or(CoreError::RootNonexistent))
}

pub fn maybe_get_root(
    config: &Config,
    source: RepoSource,
) -> Result<Option<FileMetadata>, CoreError> {
    match root_repo::maybe_get(config)? {
        Some(id) => maybe_get_metadata(config, source, id),
        None => Ok(None),
    }
}

pub fn get_all_metadata(
    config: &Config,
    source: RepoSource,
) -> Result<Vec<FileMetadata>, CoreError> {
    get_with_descendants(config, source, root_repo::get(config)?)
}

// note: does not include target file
pub fn get_children(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Vec<FileMetadata>, CoreError> {
    Ok(get_all_metadata(config, source)?
        .into_iter()
        .filter(|file| file.parent == id && file.parent != file.id)
        .collect::<Vec<FileMetadata>>())
}

pub fn get_document(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<EncryptedDocument, CoreError> {
    maybe_get_document(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_document(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<EncryptedDocument>, CoreError> {
    if maybe_get_metadata(config, source, id)?.is_none() {
        Ok(None)
    } else {
        let maybe_local = document_repo::maybe_get(config, RepoSource::Local, id)?;
        let maybe_remote = document_repo::maybe_get(config, RepoSource::Remote, id)?;
        Ok(RepoState::from_local_and_remote(maybe_local, maybe_remote)
            .and_then(|s| s.source(source)))
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
    Ok(get_all_metadata(config, RepoSource::Local)?
        .into_iter()
        .map(|f| document_repo::maybe_get(config, RepoSource::Local, f.id).map(|r| r.map(|_| f.id)))
        .collect::<Result<Vec<Option<Uuid>>, CoreError>>()?
        .into_iter()
        .filter_map(|id| id)
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
        if let Some(_) = maybe_get_document(config, source, file.id)? {
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
    let all_include_deleted = get_all_metadata_include_deleted(config, RepoSource::Local)?;
    let all_exclude_deleted = get_all_metadata(config, RepoSource::Local)?;
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
