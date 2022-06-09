use crate::internal;
use crate::keys::{file, owned_files, size};
use crate::ServerError;
use crate::ServerError::{ClientError, InternalError};
use crate::{keys, RequestContext};
use deadpool_redis::Connection;
use std::collections::HashMap;

use crate::content::document_service;
use crate::file_service::OwnershipCheck::{FileMissing, NotOwned};
use deadpool_redis::redis::AsyncCommands;
use libsecp256k1::PublicKey;
use lockbook_crypto::clock_service::get_time;
use lockbook_models::api::FileMetadataUpsertsError::{
    GetUpdatesRequired, NewFileHasOldParentAndName, NotPermissioned, RootImmutable,
};
use lockbook_models::api::*;
use lockbook_models::file_metadata::FileType::Document;
use lockbook_models::file_metadata::{EncryptedFileMetadata, FileMetadataDiff, Owner};
use lockbook_models::tree::{FileMetaExt, TEMP_FileMetaExt};
use log::info;
use redis_utils::converters::{JsonGet, PipelineJsonSet};
use redis_utils::TxError::Abort;
use redis_utils::{tx, TxError};
use uuid::Uuid;

pub async fn upsert_file_metadata(
    context: RequestContext<'_, FileMetadataUpsertsRequest>,
) -> Result<(), ServerError<FileMetadataUpsertsError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let owner = Owner(context.public_key);
    check_for_changed_root(&request.updates)?;

    let mut con = server_state.index_db_pool.get().await?;
    let mut docs_to_delete: Vec<EncryptedFileMetadata> = vec![];
    let tx = tx!(&mut con, pipe, &[owned_files(&context.public_key)], {
        let now = get_time().0 as u64;
        let files: Vec<Uuid> = con
            .maybe_json_get(owned_files(&context.public_key))
            .await?
            .ok_or(Abort(ClientError(FileMetadataUpsertsError::UserNotFound)))?;
        let keys: Vec<String> = files.into_iter().map(keys::file).collect();
        let files: Vec<EncryptedFileMetadata> = con.watch_json_mget(keys).await?;
        let mut files = files.to_map();

        docs_to_delete = apply_changes(&mut con, now, &owner, &request.updates, &mut files).await?;

        files
            .verify_integrity()
            .map_err(|_| Abort(ClientError(GetUpdatesRequired)))?;

        for (_, the_file) in &files {
            pipe.json_set(file(the_file.id), the_file)?;
            if the_file.deleted && the_file.file_type == Document {
                pipe.del(size(the_file.id));
            }
        }
        pipe.json_set(owned_files(&context.public_key), files.ids())?;
        Ok(&mut pipe)
    });
    return_if_error!(tx);

    for file in docs_to_delete {
        document_service::background_delete(server_state, file.id, file.content_version).await?;
    }
    Ok(())
}

async fn check_uniqueness(
    con: &mut Connection, new_files: &[EncryptedFileMetadata],
) -> Result<(), TxError<ServerError<FileMetadataUpsertsError>>> {
    if !new_files.is_empty() {
        let ids: Vec<String> = new_files.iter().map(keys::meta).collect();
        let existing_files: i32 = con.exists(ids).await?;
        if existing_files == 0 {
            Ok(())
        } else {
            Err(Abort(ClientError(NotPermissioned)))
        }
    } else {
        Ok(())
    }
}

fn check_for_changed_root(
    changes: &[FileMetadataDiff],
) -> Result<(), ServerError<FileMetadataUpsertsError>> {
    for change in changes {
        if let Some((old_parent, _)) = change.old_parent_and_name {
            if change.id == old_parent {
                return Err(ClientError(RootImmutable));
            }
            if change.id == change.new_parent {
                // TODO could be createdRoot
                return Err(ClientError(GetUpdatesRequired));
            }
        }
    }
    Ok(())
}

async fn apply_changes(
    con: &mut Connection, now: u64, owner: &Owner, changes: &[FileMetadataDiff],
    metas: &mut HashMap<Uuid, EncryptedFileMetadata>,
) -> Result<Vec<EncryptedFileMetadata>, TxError<ServerError<FileMetadataUpsertsError>>> {
    let mut deleted_documents = vec![];
    let mut new_files = vec![];
    for change in changes {
        match metas.maybe_find_mut(change.id) {
            Some(meta) => {
                meta.deleted = change.new_deleted;

                if let Some((old_parent, old_name)) = &change.old_parent_and_name {
                    if meta.parent != *old_parent || meta.name != *old_name {
                        return Err(Abort(ClientError(GetUpdatesRequired)));
                    }
                } else {
                    // You authored a file, and you pushed it to the server, and failed to record the change
                    // And now you think this is still a new file, so you get updates
                    return Err(Abort(ClientError(GetUpdatesRequired)));
                }
                meta.parent = change.new_parent;
                meta.name = change.new_name.clone();
                meta.folder_access_keys = change.new_folder_access_keys.clone();
                meta.metadata_version = now;

                if change.new_deleted && meta.file_type == Document {
                    deleted_documents.push(meta.clone());
                }
            }
            None => {
                if change.old_parent_and_name.is_some() {
                    return Err(Abort(ClientError(NewFileHasOldParentAndName)));
                }
                let new_meta = new_meta(now, change, owner);
                metas.insert(new_meta.id, new_meta.clone());
                new_files.push(new_meta);
            }
        }
    }

    check_uniqueness(con, &new_files).await?;

    let implicitly_deleted_ids = metas
        .filter_deleted()
        .map_err(|_| Abort(ClientError(GetUpdatesRequired)))? // TODO this could be more descriptive
        .into_iter()
        .filter(|(_, f)| !f.deleted)
        .map(|(_, f)| f.id);

    for id in implicitly_deleted_ids {
        if let Some(implicitly_deleted) = metas.maybe_find_mut(id) {
            implicitly_deleted.deleted = true;
            implicitly_deleted.metadata_version = now;
            if implicitly_deleted.file_type == Document {
                deleted_documents.push(implicitly_deleted.clone());
            }
        }
    }

    Ok(deleted_documents)
}

fn new_meta(now: u64, diff: &FileMetadataDiff, owner: &Owner) -> EncryptedFileMetadata {
    EncryptedFileMetadata {
        id: diff.id,
        file_type: diff.file_type,
        parent: diff.new_parent,
        name: diff.new_name.clone(),
        owner: owner.clone(),
        metadata_version: now,
        content_version: 0,
        deleted: diff.new_deleted,
        user_access_keys: Default::default(),
        folder_access_keys: diff.new_folder_access_keys.clone(),
    }
}

/// Changes the content and size of a document
/// Grabs the file out of redis and does some preliminary checks regarding ownership and version
/// TODO After billing, do actual space checks
pub async fn change_document_content(
    context: RequestContext<'_, ChangeDocumentContentRequest>,
) -> Result<ChangeDocumentContentResponse, ServerError<ChangeDocumentContentError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut con = server_state.index_db_pool.get().await?;

    check_ownership(&mut con, request.id, &context.public_key)
        .await
        .map_err(|err| match err {
            ClientError(OwnershipCheck::FileMissing) => {
                ClientError(ChangeDocumentContentError::DocumentNotFound)
            }
            ClientError(OwnershipCheck::NotOwned) => {
                ClientError(ChangeDocumentContentError::NotPermissioned)
            }
            ServerError::InternalError(err) => InternalError(err),
        })?;

    let watched_keys = &[file(request.id), size(request.id)];
    let new_version = get_time().0 as u64;

    let mut old_content_version = 0;

    document_service::create(server_state, request.id, new_version, &request.new_content).await?;

    let tx: Result<(), _> = tx!(&mut con, pipe, watched_keys, {
        let new_size =
            FileUsage { file_id: request.id, size_bytes: request.new_content.value.len() as u64 };
        let mut meta: EncryptedFileMetadata = con
            .maybe_json_get(file(request.id))
            .await?
            .ok_or(Abort(ClientError(ChangeDocumentContentError::DocumentNotFound)))?;

        if meta.deleted {
            return Err(Abort(ClientError(ChangeDocumentContentError::DocumentDeleted)));
        }

        if false {
            return Err(Abort(ClientError(ChangeDocumentContentError::NotPermissioned)));
        }

        if request.old_metadata_version != meta.metadata_version {
            return Err(Abort(ClientError(ChangeDocumentContentError::EditConflict)));
        }

        old_content_version = meta.content_version;

        meta.content_version = new_version;
        meta.metadata_version = new_version;

        pipe.json_set(size(request.id), new_size)?
            .json_set(file(request.id), meta)
    });
    if tx.is_err() {
        // Cleanup the NEW file created if, for some reason, the tx failed
        document_service::background_delete(server_state, request.id, new_version).await?;
    }
    return_if_error!(tx);

    document_service::background_delete(server_state, request.id, old_content_version).await?;

    Ok(ChangeDocumentContentResponse { new_content_version: new_version })
}

#[derive(Debug)]
pub enum OwnershipCheck {
    FileMissing,
    NotOwned,
}

async fn check_ownership(
    con: &mut Connection, id: Uuid, pk: &PublicKey,
) -> Result<(), ServerError<OwnershipCheck>> {
    con.maybe_json_get(file(id))
        .await?
        .map(|meta: EncryptedFileMetadata| meta.owner.0)
        .map(|pk1| pk1 == *pk)
        .map(|is_owner| if is_owner { Ok(()) } else { Err(ClientError(NotOwned)) })
        .ok_or(ClientError(FileMissing))?
}

pub async fn get_document(
    context: RequestContext<'_, GetDocumentRequest>,
) -> Result<GetDocumentResponse, ServerError<GetDocumentError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut con = server_state.index_db_pool.get().await?;
    check_ownership(&mut con, request.id, &context.public_key)
        .await
        .map_err(|err| match err {
            ClientError(OwnershipCheck::FileMissing) => {
                ClientError(GetDocumentError::DocumentNotFound)
            }
            ClientError(OwnershipCheck::NotOwned) => ClientError(GetDocumentError::NotPermissioned),
            ServerError::InternalError(err) => InternalError(err),
        })?;
    let content = document_service::get(server_state, request.id, request.content_version).await?;
    Ok(GetDocumentResponse { content })
}

pub async fn get_updates(
    context: RequestContext<'_, GetUpdatesRequest>,
) -> Result<GetUpdatesResponse, ServerError<GetUpdatesError>> {
    let (request, _server_state) = (&context.request, context.server_state);
    let mut con = context.server_state.index_db_pool.get().await?;
    let files: Vec<Uuid> = con
        .maybe_json_get(owned_files(&context.public_key))
        .await?
        .ok_or(ClientError(GetUpdatesError::UserNotFound))?;
    let keys: Vec<String> = files.into_iter().map(keys::file).collect();
    let files: Vec<EncryptedFileMetadata> = con.watch_json_mget(keys).await?;

    let file_metadata = files
        .into_iter()
        .filter(|meta| meta.metadata_version > request.since_metadata_version)
        .collect();

    info!("{:?}", file_metadata);

    Ok(GetUpdatesResponse { file_metadata })
}
