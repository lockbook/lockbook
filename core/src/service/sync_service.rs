use std::fmt;

use crate::model::client_conversion::ClientWorkUnit;
use crate::model::document_type::DocumentType;
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::account_repo;
use crate::repo::file_repo;
use crate::service::{file_encryption_service, file_service};
use crate::CoreError;
use crate::{client, utils};
use lockbook_models::account::Account;
use lockbook_models::api::{
    ChangeDocumentContentRequest, FileMetadataUpsertsRequest, GetDocumentRequest, GetUpdatesRequest,
};
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata, FileType};
use lockbook_models::work_unit::WorkUnit;
use serde::Serialize;

use super::file_compression_service;

#[derive(Debug, Serialize, Clone)]
pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub most_recent_update_from_server: u64,
}

pub struct SyncProgress {
    pub total: usize,
    pub progress: usize,
    pub current_work_unit: ClientWorkUnit,
}

pub fn calculate_work(config: &Config) -> Result<WorkCalculated, CoreError> {
    info!("Calculating Work");

    let account = account_repo::get(config)?;
    let base_metadata = file_repo::get_all_metadata(config, RepoSource::Base)?;
    let base_max_metadata_version = base_metadata
        .iter()
        .map(|f| f.metadata_version)
        .max()
        .unwrap_or(0);
    println!("max base version: {}", base_max_metadata_version);

    let server_updates = client::request(
        &account,
        GetUpdatesRequest {
            since_metadata_version: base_max_metadata_version,
        },
    )
    .map_err(CoreError::from)?
    .file_metadata;

    calculate_work_from_updates(config, &server_updates, base_max_metadata_version)
}

fn calculate_work_from_updates(
    config: &Config,
    server_updates: &[FileMetadata],
    mut last_sync: u64,
) -> Result<WorkCalculated, CoreError> {
    let mut work_units: Vec<WorkUnit> = vec![];
    let all_metadata = file_repo::get_all_metadata_with_encrypted_changes(
        config,
        RepoSource::Local,
        server_updates,
    )?;
    for metadata in server_updates {
        if metadata.metadata_version > last_sync {
            last_sync = metadata.metadata_version;
        }

        match file_repo::maybe_get_metadata(config, RepoSource::Local, metadata.id)? {
            None => {
                if !metadata.deleted {
                    // no work for files we don't have that have been deleted
                    // trace!("calculate_work remote new file: {:#?}", metadata);
                    work_units.push(WorkUnit::ServerChange {
                        metadata: utils::find(&all_metadata, metadata.id)?,
                    })
                }
            }
            Some(local_metadata) => {
                if metadata.metadata_version != local_metadata.metadata_version {
                    // trace!("calculate_work remote updated file: {:#?}\n(local version = {:#?})", metadata, local_metadata);
                    work_units.push(WorkUnit::ServerChange {
                        metadata: utils::find(&all_metadata, metadata.id)?,
                    })
                }
            }
        };
    }

    work_units.sort_by(|f1, f2| {
        f1.get_metadata()
            .metadata_version
            .cmp(&f2.get_metadata().metadata_version)
    });

    for file_diff in file_repo::get_all_metadata_changes(config)? {
        // trace!("calculate_work local metadata change: {:#?}", file_diff);
        let metadata = file_repo::get_metadata(config, RepoSource::Local, file_diff.id)?;
        work_units.push(WorkUnit::LocalChange { metadata });
    }
    for doc_id in file_repo::get_all_with_document_changes(config)? {
        // trace!("calculate_work local document change: {:#?}", doc_id);
        let metadata = file_repo::get_metadata(config, RepoSource::Local, doc_id)?;
        work_units.push(WorkUnit::LocalChange { metadata });
    }
    debug!("Work Calculated: {:#?}", work_units);

    Ok(WorkCalculated {
        work_units,
        most_recent_update_from_server: last_sync,
    })
}

#[derive(PartialEq, Debug)]
pub enum MaybeMergeResult<T> {
    Resolved(T),
    Conflict { base: T, local: T, remote: T },
    BaselessConflict { local: T, remote: T },
}

fn merge_maybe<T>(
    maybe_base: Option<T>,
    maybe_local: Option<T>,
    maybe_remote: Option<T>,
) -> Result<MaybeMergeResult<T>, CoreError> {
    Ok(MaybeMergeResult::Resolved(
        match (maybe_base, maybe_local, maybe_remote) {
            (None, None, None) => {
                // improper call of this function
                return Err(CoreError::Unexpected(String::from(
                    "3-way maybe merge with none of the 3",
                )));
            }
            (None, None, Some(remote)) => {
                // new from remote
                remote
            }
            (None, Some(local), None) => {
                // new from local
                local
            }
            (None, Some(local), Some(remote)) => {
                // Every once in a while, a lockbook client successfully syncs a file to server then gets interrupted
                // before noting the successful sync. The next time that client pushes they're required to pull first
                // and the next time they pull they'll merge the local and remote version of the file with no base.
                // It's possible there have been changes made by other clients in the meantime, but we do the best we
                // can to produce a reasonable result.
                return Ok(MaybeMergeResult::BaselessConflict { local, remote });
            }
            (Some(base), None, None) => {
                // no changes
                base
            }
            (Some(_base), None, Some(remote)) => {
                // remote changes
                remote
            }
            (Some(_base), Some(local), None) => {
                // local changes
                local
            }
            (Some(base), Some(local), Some(remote)) => {
                // conflict
                return Ok(MaybeMergeResult::Conflict {
                    base,
                    local,
                    remote,
                });
            }
        },
    ))
}

fn merge_metadata(
    base: DecryptedFileMetadata,
    local: DecryptedFileMetadata,
    remote: DecryptedFileMetadata,
) -> DecryptedFileMetadata {
    let local_renamed = local.decrypted_name != base.decrypted_name;
    let remote_renamed = remote.decrypted_name != base.decrypted_name;
    let decrypted_name = match (local_renamed, remote_renamed) {
        (false, false) => base.decrypted_name,
        (true, false) => local.decrypted_name,
        (false, true) => remote.decrypted_name,
        (true, true) => remote.decrypted_name, // resolve rename conflicts in favor of remote
    };

    let local_moved = local.parent != base.parent;
    let remote_moved = remote.parent != base.parent;
    let parent = match (local_moved, remote_moved) {
        (false, false) => base.parent,
        (true, false) => local.parent,
        (false, true) => remote.parent,
        (true, true) => remote.parent, // resolve move conflicts in favor of remote
    };

    DecryptedFileMetadata {
        id: base.id,               // ids never change
        file_type: base.file_type, // file types never change
        parent,
        decrypted_name,
        owner: base.owner,                         // owners never change
        metadata_version: remote.metadata_version, // resolve metadata version conflicts in favor of remote
        content_version: remote.content_version, // resolve content version conflicts in favor of remote
        deleted: base.deleted || local.deleted || remote.deleted, // resolve delete conflicts by deleting
        decrypted_access_key: base.decrypted_access_key,          // access keys never change
    }
}

fn merge_maybe_metadata(
    maybe_base: Option<DecryptedFileMetadata>,
    maybe_local: Option<DecryptedFileMetadata>,
    maybe_remote: Option<DecryptedFileMetadata>,
) -> Result<DecryptedFileMetadata, CoreError> {
    Ok(match merge_maybe(maybe_base, maybe_local, maybe_remote)? {
        MaybeMergeResult::Resolved(merged) => merged,
        MaybeMergeResult::Conflict {
            base,
            local,
            remote,
        } => merge_metadata(base, local, remote),
        MaybeMergeResult::BaselessConflict {
            local: _local,
            remote,
        } => remote,
    })
}

fn merge_maybe_documents(
    merged_metadata: &DecryptedFileMetadata,
    remote_metadata: &DecryptedFileMetadata,
    maybe_base_document: Option<DecryptedDocument>,
    maybe_local_document: Option<DecryptedDocument>,
    remote_document: DecryptedDocument,
) -> Result<ResolvedDocument, CoreError> {
    match maybe_base_document {
        Some(ref base_document) => trace!(
            "\nget_resolved_document base: {:#?}",
            String::from_utf8_lossy(&base_document)
        ),
        None => trace!("\nget_resolved_document base: None"),
    }
    match maybe_local_document {
        Some(ref local_document) => trace!(
            "\nget_resolved_document local: {:#?}",
            String::from_utf8_lossy(&local_document)
        ),
        None => trace!("\nget_resolved_document local: None"),
    }
    trace!(
        "\nget_resolved_document remote: {:#?}",
        String::from_utf8_lossy(&remote_document)
    );
    let result = Ok(
        match merge_maybe(
            maybe_base_document,
            maybe_local_document,
            Some(remote_document.clone()),
        )? {
            MaybeMergeResult::Resolved(merged_document) => ResolvedDocument::Merged {
                remote_metadata: remote_metadata.clone(),
                remote_document,
                merged_metadata: merged_metadata.clone(),
                merged_document,
            },
            MaybeMergeResult::Conflict {
                base: base_document,
                local: local_document,
                remote: remote_document,
            } => {
                match DocumentType::from_file_name_using_extension(&merged_metadata.decrypted_name)
                {
                    // text documents get 3-way merged
                    DocumentType::Text => {
                        let merged_document = match diffy::merge_bytes(
                            &base_document,
                            &local_document,
                            &remote_document,
                        ) {
                            Ok(without_conflicts) => without_conflicts,
                            Err(with_conflicts) => with_conflicts,
                        };
                        ResolvedDocument::Merged {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            merged_metadata: merged_metadata.clone(),
                            merged_document,
                        }
                    }
                    // other documents have local version copied to new file
                    DocumentType::Drawing | DocumentType::Other => {
                        let mut copied_local_metadata = file_service::create(
                        FileType::Document,
                        merged_metadata.parent,
                        "this is overwritten two statements down because we need the uuid generated by this function call",
                        &merged_metadata.owner,
                    );
                        let conflict_name = format!(
                            "{}-CONTENT-CONFLICT-{}",
                            copied_local_metadata.id.clone(),
                            &merged_metadata.decrypted_name,
                        );
                        copied_local_metadata.decrypted_name = conflict_name;

                        ResolvedDocument::Copied {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            copied_local_metadata: merged_metadata.clone(),
                            copied_local_document: local_document,
                        }
                    }
                }
            }
            MaybeMergeResult::BaselessConflict {
                local: local_document,
                remote: remote_document,
            } => {
                match DocumentType::from_file_name_using_extension(&merged_metadata.decrypted_name)
                {
                    // text documents get 3-way merged
                    DocumentType::Text => {
                        let merged_document =
                            match diffy::merge_bytes(&[], &local_document, &remote_document) {
                                Ok(without_conflicts) => without_conflicts,
                                Err(with_conflicts) => with_conflicts,
                            };
                        ResolvedDocument::Merged {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            merged_metadata: merged_metadata.clone(),
                            merged_document,
                        }
                    }
                    // other documents have local version copied to new file
                    DocumentType::Drawing | DocumentType::Other => {
                        let mut copied_local_metadata = file_service::create(
                        FileType::Document,
                        merged_metadata.parent,
                        "this is overwritten two statements down because we need the uuid generated by this function call",
                        &merged_metadata.owner,
                    );
                        let conflict_name = format!(
                            "{}-CONTENT-CONFLICT-{}",
                            copied_local_metadata.id.clone(),
                            &merged_metadata.decrypted_name,
                        );
                        copied_local_metadata.decrypted_name = conflict_name;

                        ResolvedDocument::Copied {
                            remote_metadata: remote_metadata.clone(),
                            remote_document,
                            copied_local_metadata: merged_metadata.clone(),
                            copied_local_document: local_document,
                        }
                    }
                }
            }
        },
    );

    trace!("\nget_resolved_document merged: {:#?}", result);

    result
}

enum ResolvedDocument {
    Merged {
        remote_metadata: DecryptedFileMetadata,
        remote_document: DecryptedDocument,
        merged_metadata: DecryptedFileMetadata,
        merged_document: DecryptedDocument,
    },
    Copied {
        remote_metadata: DecryptedFileMetadata,
        remote_document: DecryptedDocument,
        copied_local_metadata: DecryptedFileMetadata,
        copied_local_document: DecryptedDocument,
    },
}

impl fmt::Debug for ResolvedDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolvedDocument::Merged {
                remote_metadata,
                remote_document,
                merged_metadata,
                merged_document,
            } => f
                .debug_struct("ResolvedDocument::Merged")
                .field("remote_metadata", remote_metadata)
                .field("remote_document", &String::from_utf8_lossy(remote_document))
                .field("merged_metadata", merged_metadata)
                .field("merged_document", &String::from_utf8_lossy(merged_document))
                .finish(),
            ResolvedDocument::Copied {
                remote_metadata,
                remote_document,
                copied_local_metadata,
                copied_local_document,
            } => f
                .debug_struct("ResolvedDocument::Copied")
                .field("remote_metadata", remote_metadata)
                .field("remote_document", &String::from_utf8_lossy(remote_document))
                .field("copied_local_metadata", copied_local_metadata)
                .field(
                    "copied_local_document",
                    &String::from_utf8_lossy(copied_local_document),
                )
                .finish(),
        }
    }
}

/// Gets a resolved document based on merge of local, base, and remote. Some document types are 3-way merged; others
/// have old contents copied to a new file. Remote document is returned so that caller can update base.
fn get_resolved_document(
    config: &Config,
    account: &Account,
    remote_metadatum: &DecryptedFileMetadata,
    merged_metadatum: &DecryptedFileMetadata,
) -> Result<Option<ResolvedDocument>, CoreError> {
    let maybe_remote_document_encrypted = client::request(
        &account,
        GetDocumentRequest {
            id: remote_metadatum.id,
            content_version: remote_metadatum.content_version, // todo: is this content_version is incorrect?
        },
    )?
    .content;
    let maybe_remote_document = match maybe_remote_document_encrypted {
        Some(remote_document_encrypted) => Some(file_compression_service::decompress(
            &file_encryption_service::decrypt_document(
                &remote_document_encrypted,
                &remote_metadatum,
            )?,
        )?),
        None => None,
    };

    match maybe_remote_document {
        Some(ref remote_document) => trace!(
            "\npulled document: {:#?}",
            String::from_utf8_lossy(&remote_document)
        ),
        None => trace!("\npulled document: None"),
    };
    trace!("\nmetadata of pulled document: {:#?}", remote_metadatum);

    let maybe_document_state = file_repo::maybe_get_document_state(config, remote_metadatum.id)?;
    let (maybe_base_document, maybe_local_document) = match maybe_document_state {
        Some(document_state) => (document_state.clone().base(), Some(document_state.local())),
        None => (None, None),
    };

    match maybe_remote_document {
        Some(remote_document) => {
            // update remote repo to version from server
            file_repo::insert_document(
                config,
                RepoSource::Base,
                &remote_metadatum,
                &remote_document,
            )?;

            // merge document content for documents with updated content
            let merged_document = merge_maybe_documents(
                merged_metadatum,
                remote_metadatum,
                maybe_base_document,
                maybe_local_document,
                remote_document,
            )?;

            Ok(Some(merged_document))
        }
        None => Ok(None),
    }
}

/// Updates local files to 3-way merge of local, base, and remote; updates base files to remote.
fn pull(
    config: &Config,
    account: &Account,
    _f: &Option<Box<dyn Fn(SyncProgress)>>,
) -> Result<(), CoreError> {
    let base_metadata = file_repo::get_all_metadata(config, RepoSource::Base)?;
    let base_max_metadata_version = base_metadata
        .iter()
        .map(|f| f.metadata_version)
        .max()
        .unwrap_or(0);

    let remote_metadata_changes = client::request(
        account,
        GetUpdatesRequest {
            since_metadata_version: base_max_metadata_version,
        },
    )
    .map_err(CoreError::from)?
    .file_metadata;

    let local_metadata = file_repo::get_all_metadata(config, RepoSource::Local)?;
    let remote_metadata = file_repo::get_all_metadata_with_encrypted_changes(
        config,
        RepoSource::Base,
        &remote_metadata_changes,
    )?;

    let mut base_metadata_updates = Vec::new();
    let mut base_document_updates = Vec::new();
    let mut local_metadata_updates = Vec::new();
    let mut local_document_updates = Vec::new();

    // iterate changes
    for encrypted_remote_metadatum in remote_metadata_changes {
        // merge metadata
        let remote_metadatum = utils::find(&remote_metadata, encrypted_remote_metadatum.id)?;
        let maybe_base_metadatum = utils::maybe_find(&base_metadata, encrypted_remote_metadatum.id);
        let maybe_local_metadatum =
            utils::maybe_find(&local_metadata, encrypted_remote_metadatum.id);

        trace!("merge_maybe_metadata base: {:#?}", maybe_base_metadatum);
        trace!("merge_maybe_metadata local: {:#?}", maybe_local_metadatum);
        trace!(
            "merge_maybe_metadata remote: {:#?}",
            Some(remote_metadatum.clone())
        );
        let merged_metadatum = merge_maybe_metadata(
            maybe_base_metadatum.clone(),
            maybe_local_metadatum,
            Some(remote_metadatum.clone()),
        )?;
        trace!("merge_maybe_metadata result: {:#?}\n", merged_metadatum);
        base_metadata_updates.push(remote_metadatum.clone()); // update base to remote
        local_metadata_updates.push(merged_metadatum.clone()); // update local to merged

        // merge document content
        let content_updated = remote_metadatum.file_type == FileType::Document
            && if let Some(base) = maybe_base_metadatum {
                remote_metadatum.content_version != base.content_version
            } else {
                true
            };
        if content_updated {
            match get_resolved_document(config, account, &remote_metadatum, &merged_metadatum)? {
                Some(ResolvedDocument::Merged {
                    remote_metadata,
                    remote_document,
                    merged_metadata,
                    merged_document,
                }) => {
                    base_document_updates.push((remote_metadata, remote_document)); // update base to remote
                    local_document_updates.push((merged_metadata, merged_document));
                    // update local to merged
                }
                Some(ResolvedDocument::Copied {
                    remote_metadata,
                    remote_document,
                    copied_local_metadata,
                    copied_local_document,
                }) => {
                    base_document_updates.push((remote_metadata, remote_document)); // update base to remote
                    local_metadata_updates.push(copied_local_metadata.clone()); // new local metadata from merge
                    local_document_updates.push((copied_local_metadata, copied_local_document));
                    // new local document from merge
                }
                None => {}
            }
        }
    }

    // resolve path conflicts
    for path_conflict in file_service::get_path_conflicts(&local_metadata, &local_metadata_updates)?
    {
        let to_rename = utils::find_mut(&mut local_metadata_updates, path_conflict.staged)?;
        let conflict_name = format!(
            "{}-PATH-CONFLICT-{}",
            to_rename.id.clone(),
            &to_rename.decrypted_name,
        );
        to_rename.decrypted_name = conflict_name;
    }

    // update base
    // trace!("\npull base_metadata_updates: {:#?}", base_metadata_updates);
    let utf8_base_document_updates: Vec<(DecryptedFileMetadata, String)> = base_document_updates
        .iter()
        .map(|(f, d)| (f.clone(), String::from_utf8_lossy(d).into_owned()))
        .collect();
    trace!(
        "\npull base_document_updates: {:#?}",
        utf8_base_document_updates
    );
    file_repo::insert_metadata(config, RepoSource::Base, &base_metadata_updates)?;
    for (metadata, document_update) in base_document_updates {
        file_repo::insert_document(config, RepoSource::Base, &metadata, &document_update)?;
    }

    // update local
    // trace!("\npull local_metadata_updates: {:#?}", local_metadata_updates);
    let utf8_local_document_updates: Vec<(DecryptedFileMetadata, String)> = local_document_updates
        .iter()
        .map(|(f, d)| (f.clone(), String::from_utf8_lossy(d).into_owned()))
        .collect();
    trace!(
        "\npull local_document_updates: {:#?}",
        utf8_local_document_updates
    );
    file_repo::insert_metadata(config, RepoSource::Local, &local_metadata_updates)?;
    for (metadata, document_update) in local_document_updates {
        file_repo::insert_document(config, RepoSource::Local, &metadata, &document_update)?;
    }

    Ok(())
}

/// Updates remote and base metadata to local.
fn push_metadata(
    config: &Config,
    account: &Account,
    _f: &Option<Box<dyn Fn(SyncProgress)>>,
) -> Result<(), CoreError> {
    trace!(
        "\npush_metadata all_metadata_changes: {:#?}",
        file_repo::get_all_metadata_changes(config)?
    );
    // update remote to local (metadata)
    let metadata_changes = file_repo::get_all_metadata_changes(config)?;
    if metadata_changes.len() != 0 {
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: metadata_changes,
            },
        )
        .map_err(CoreError::from)?;
    }

    // update base to local
    file_repo::promote_metadata(config)?;
    // trace!("push_metadata end");

    Ok(())
}

/// Updates remote and base files to local.
fn push_documents(
    config: &Config,
    account: &Account,
    _f: &Option<Box<dyn Fn(SyncProgress)>>,
) -> Result<(), CoreError> {
    for id in file_repo::get_all_with_document_changes(config)? {
        let local_metadata = file_repo::get_metadata(config, RepoSource::Local, id)?;
        let local_content = file_repo::get_document(config, RepoSource::Local, id)?;
        trace!(
            "\npushed document: {:#?}",
            String::from_utf8_lossy(&local_content)
        );
        let encrypted_content = file_encryption_service::encrypt_document(
            &file_compression_service::compress(&local_content)?,
            &local_metadata,
        )?;

        // update remote to local (document)
        client::request(
            &account,
            ChangeDocumentContentRequest {
                id: id,
                old_metadata_version: local_metadata.metadata_version,
                new_content: encrypted_content,
            },
        )
        .map_err(CoreError::from)?;
    }

    // update base to local
    file_repo::promote_documents(config)?;

    Ok(())
}

pub fn sync(config: &Config, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), CoreError> {
    trace!("sync start");
    let account = &account_repo::get(config)?;
    trace!("  sync pull");
    pull(config, account, &f)?;
    trace!("  sync push_metadata");
    push_metadata(config, account, &f)?;
    trace!("  sync pull");
    pull(config, account, &f)?;
    trace!("  sync push_documents");
    push_documents(config, account, &f)?;
    file_repo::prune_deleted(config)?;
    trace!("sync end");
    Ok(())
}

#[cfg(test)]
mod unit_test_sync_service {
    use std::str::FromStr;

    use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
    use uuid::Uuid;

    use crate::service::sync_service::{self, MaybeMergeResult};

    #[test]
    fn merge_maybe_resolved_base() {
        let base = Some(0);
        let local = None;
        let remote = None;

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(0));
    }

    #[test]
    fn merge_maybe_resolved_local() {
        let base = None;
        let local = Some(1);
        let remote = None;

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(1));
    }

    #[test]
    fn merge_maybe_resolved_local_with_base() {
        let base = Some(0);
        let local = Some(1);
        let remote = None;

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(1));
    }

    #[test]
    fn merge_maybe_resolved_remote() {
        let base = None;
        let local = None;
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(2));
    }

    #[test]
    fn merge_maybe_resolved_remote_with_base() {
        let base = Some(0);
        let local = None;
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(result, MaybeMergeResult::Resolved(2));
    }

    #[test]
    fn merge_maybe_resolved_conflict() {
        let base = Some(0);
        let local = Some(1);
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(
            result,
            MaybeMergeResult::Conflict {
                base: 0,
                local: 1,
                remote: 2
            }
        );
    }

    #[test]
    fn merge_maybe_resolved_baseless_conflict() {
        let base = None;
        let local = Some(1);
        let remote = Some(2);

        let result = sync_service::merge_maybe(base, local, remote).unwrap();

        assert_eq!(
            result,
            MaybeMergeResult::BaselessConflict {
                local: 1,
                remote: 2
            }
        );
    }

    #[test]
    fn merge_maybe_none() {
        let base = None;
        let local = None;
        let remote = None;

        sync_service::merge_maybe::<i32>(base, local, remote).unwrap_err();
    }

    #[test]
    fn merge_metadata_local_and_remote_moved() {
        let base = DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("a33b99e8-140d-4a74-b564-f72efdcb5b3a").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        };
        let local = DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c13f10f7-9360-4dd2-8b3a-0891a81c8bf8").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        };
        let remote = DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786756,
            content_version: 1634693786556,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        };

        let result = sync_service::merge_metadata(base, local, remote);

        assert_eq!(
            result,
            DecryptedFileMetadata {
                id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
                file_type: FileType::Document,
                parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
                decrypted_name: String::from("test.txt"),
                metadata_version: 1634693786756,
                content_version: 1634693786556,
                deleted: false,
                owner: Default::default(),
                decrypted_access_key: Default::default(),
            }
        );
    }

    #[test]
    fn merge_maybe_metadata_local_and_remote_moved() {
        let base = Some(DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("a33b99e8-140d-4a74-b564-f72efdcb5b3a").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        });
        let local = Some(DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c13f10f7-9360-4dd2-8b3a-0891a81c8bf8").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786444,
            content_version: 1634693786444,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        });
        let remote = Some(DecryptedFileMetadata {
            id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
            file_type: FileType::Document,
            parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
            decrypted_name: String::from("test.txt"),
            metadata_version: 1634693786756,
            content_version: 1634693786556,
            deleted: false,
            owner: Default::default(),
            decrypted_access_key: Default::default(),
        });

        let result = sync_service::merge_maybe_metadata(base, local, remote).unwrap();

        assert_eq!(
            result,
            DecryptedFileMetadata {
                id: Uuid::from_str("db63957b-3e52-410c-8e5e-66db2619fb33").unwrap(),
                file_type: FileType::Document,
                parent: Uuid::from_str("c52d8737-0a89-45aa-8411-b74e0dd71470").unwrap(),
                decrypted_name: String::from("test.txt"),
                metadata_version: 1634693786756,
                content_version: 1634693786556,
                deleted: false,
                owner: Default::default(),
                decrypted_access_key: Default::default(),
            }
        );
    }
}
