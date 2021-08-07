use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::account_repo;
use crate::repo::digest_repo;
use crate::repo::document_repo;
use crate::repo::metadata_repo;
use crate::service::file_compression_service;
use crate::service::file_encryption_service;
use crate::utils;
use crate::utils::slices_equal;
use crate::CoreError;
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::DecryptedFileMetadata;
use lockbook_models::file_metadata::FileMetadataDiff;
use lockbook_models::file_metadata::FileType;
use sha2::Digest;
use sha2::Sha256;
use uuid::Uuid;

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

/// Adds or updates the metadata of a file on disk.
/// Disk optimization opportunity: this function needlessly writes to disk when setting local metadata = remote metadata.
/// CPU optimization opportunity: this function needlessly decrypts all metadata rather than just ancestors of metadata parameter.
pub fn insert_metadata(
    config: &Config,
    source: RepoSource,
    metadata: &DecryptedFileMetadata,
) -> Result<(), CoreError> {
    // encrypt metadata
    let account = account_repo::get(config)?;
    let all_metadata = get_all_metadata(config, source)?;
    let parent = utils::find_parent(&all_metadata, metadata.id)?;
    let encrypted_metadata = file_encryption_service::encrypt_metadatum(
        &account,
        &parent.decrypted_access_key,
        metadata,
    )?;

    // perform insertion
    metadata_repo::insert(config, source, &encrypted_metadata)?;

    // remove local if local == remote
    if let Some(opposite) =
        metadata_repo::maybe_get(config, source.opposite(), encrypted_metadata.id)?
    {
        if slices_equal(&opposite.name.hmac, &encrypted_metadata.name.hmac)
            && opposite.parent == metadata.parent
            && opposite.deleted == metadata.deleted
        {
            metadata_repo::delete(config, RepoSource::Local, metadata.id)?;
        }
    }

    // delete documents from disk if their metadata is set to deleted
    if metadata.deleted {
        if let Some(_) = maybe_get_document(config, source, metadata.id)? {
            delete_content(config, metadata.id)?;
        }
    }

    Ok(())
}

pub fn get_metadata(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    maybe_get_metadata(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_metadata(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<DecryptedFileMetadata>, CoreError> {
    let all_metadata = get_all_metadata(config, source)?;
    Ok(utils::maybe_find(&all_metadata, id))
}

pub fn get_all_metadata(
    config: &Config,
    source: RepoSource,
) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
    let account = account_repo::get(config)?;
    let encrypted_remote = metadata_repo::get_all(config, RepoSource::Remote)?;
    let remote = file_encryption_service::decrypt_metadata(&account, &encrypted_remote)?;
    match source {
        RepoSource::Local => {
            let encrypted_local = metadata_repo::get_all(config, RepoSource::Local)?;
            let local = file_encryption_service::decrypt_metadata(&account, &encrypted_local)?;
            Ok(utils::stage(&local, &remote)
                .into_iter()
                .map(|(f, _)| f)
                .collect())
        }
        RepoSource::Remote => Ok(remote),
    }
}

/// Adds or updates the content of a document on disk.
/// Disk optimization opportunity: this function needlessly writes to disk when setting local content = remote content.
/// CPU optimization opportunity: this function needlessly decrypts all metadata rather than just ancestors of metadata parameter.
pub fn insert_document(
    config: &Config,
    source: RepoSource,
    metadata: &DecryptedFileMetadata,
    document: &[u8],
) -> Result<(), CoreError> {
    // encrypt document and compute digest
    let digest = Sha256::digest(&document);
    let compressed_document = file_compression_service::compress(&document)?;
    let encrypted_document = file_encryption_service::encrypt_document(document, &metadata)?;

    // perform insertions
    document_repo::insert(config, source, metadata.id, &encrypted_document)?;
    digest_repo::insert(config, source, metadata.id, &digest)?;

    // remove local if local == remote
    if let Some(opposite) = digest_repo::maybe_get(config, source.opposite(), metadata.id)? {
        if slices_equal(&opposite, &digest) {
            document_repo::delete(config, RepoSource::Local, metadata.id)?;
            digest_repo::delete(config, RepoSource::Local, metadata.id)?;
        }
    }

    Ok(())
}

pub fn get_document(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<DecryptedDocument, CoreError> {
    maybe_get_document(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_document(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<DecryptedDocument>, CoreError> {
    let maybe_metadata = maybe_get_metadata(config, source, id)?;
    let maybe_encrypted = document_repo::maybe_get(config, source, id)?;
    match (maybe_metadata, maybe_encrypted) {
        (Some(metadata), Some(encrypted)) => Ok(Some(file_encryption_service::decrypt_document(
            &encrypted, &metadata,
        )?)),
        _ => Ok(None),
    }
}

/// Removes metadata, content, and digests of deleted files or files with deleted ancestors. Call this function after a set of operations rather than in-between each operation because otherwise you'll prune e.g. a file that was moved out of a folder that was deleted.
pub fn prune_local_deleted(config: &Config) -> Result<(), CoreError> {
    let all_metadata = get_all_metadata(config, RepoSource::Local)?;
    let deleted_metadata = utils::filter_deleted(&all_metadata)?;
    for file in deleted_metadata {
        delete_metadata(config, file.id)?;
        if file.file_type == FileType::Document {
            delete_content(config, file.id)?;
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
