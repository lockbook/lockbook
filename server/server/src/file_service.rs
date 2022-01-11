use crate::keys::{file, owned_files, size};
use crate::ServerError::{ClientError, InternalError};
use crate::{file_content_client, ServerError};
use crate::{keys, RequestContext};

use lockbook_crypto::clock_service::get_time;
use lockbook_models::api::FileMetadataUpsertsError::{CannotMoveFolderIntoItself, RootImmutable};
use lockbook_models::api::*;
use lockbook_models::file_metadata::{
    EncryptedFileMetadata, EncryptedFileMetadataExt, FileMetadataDiff,
};
use redis_utils::converters::{JsonGet, JsonSet};
use redis_utils::TxError::Abort;
use redis_utils::{tx, TxError};
use uuid::Uuid;

pub async fn upsert_file_metadata(
    context: RequestContext<'_, FileMetadataUpsertsRequest>,
) -> Result<(), ServerError<FileMetadataUpsertsError>> {
    let (request, server_state) = (&context.request, context.server_state);
    check_for_changed_root(&request.updates)?;

    let mut con = server_state.index_db_pool.get().await?;
    let tx = tx!(&mut con, pipe, &[owned_files(&context.public_key)], {
        let files: Vec<Uuid> = con
            .maybe_json_get(owned_files(&context.public_key))
            .await?
            .ok_or(Abort(ClientError(FileMetadataUpsertsError::UserNotFound)))?;
        let keys: Vec<String> = files.into_iter().map(keys::file).collect();
        let mut files: Vec<EncryptedFileMetadata> = con.json_mget(keys).await?;

        for the_file in files {
            pipe.json_set(file(the_file.id), the_file)?;
        }
        Ok(&mut pipe)
    });
    return_if_error!(tx);
    Err(internal!(""))
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
                return Err(ClientError(CannotMoveFolderIntoItself))
            }
        }
    }
    Ok(())
}

fn apply_changes(changes: &mut [FileMetadataDiff], metas: &mut Vec<EncryptedFileMetadata>) -> Result<(), TxError<FileMetadataUpsertsError>> {
    for change in changes {
        match metas.find_mut(change.id) {
            Some(meta) => {}
            None => metas.push(EncryptedFileMetadata::new)
        }
    }
    Ok(())
}

fn new_meta(diff: FileMetadataDiff) -> EncryptedFileMetadata {
    EncryptedFileMetadata {
        id: diff.id,
        file_type: diff.file_type,
        parent: diff.new_parent,
        name: diff.new_name,
        owner: diff.owner,
        metadata_version: (get_time().0 as u64),
        content_version: 0, // TODO what should this be
        deleted: diff.new_deleted,
        user_access_keys: Default::default(),
        folder_access_keys: diff.new_folder_access_keys
    }
}

/// Changes the content and size of a document
/// Grabs the file out of redis and does some preliminary checks regarding ownership and version
/// TODO After #949 do actual ownership checks
/// TODO After billing, do actual space checks
pub async fn change_document_content(
    context: RequestContext<'_, ChangeDocumentContentRequest>,
) -> Result<ChangeDocumentContentResponse, ServerError<ChangeDocumentContentError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut con = server_state.index_db_pool.get().await?;

    let watched_keys = &[file(request.id), size(request.id)];
    let mut new_version = 0;
    let new_size = request.new_content.value.len() as u64;

    let tx = tx!(&mut con, pipe, watched_keys, {
        new_version = get_time().0 as u64;
        let mut meta: EncryptedFileMetadata =
            con.maybe_json_get(file(request.id))
                .await?
                .ok_or(Abort(ClientError(
                    ChangeDocumentContentError::DocumentNotFound,
                )))?;

        if meta.deleted {
            return Err(Abort(ClientError(
                ChangeDocumentContentError::DocumentDeleted,
            )));
        }

        if !meta.owner.is_empty() {
            return Err(Abort(ClientError(
                ChangeDocumentContentError::NotPermissioned,
            )));
        }

        if request.old_metadata_version != meta.content_version {
            return Err(Abort(ClientError(ChangeDocumentContentError::EditConflict)));
        }

        meta.content_version = new_version;

        pipe.set(size(request.id), new_size)
            .json_set(file(request.id), meta)
    });
    return_if_error!(tx);

    file_content_client::create(
        &server_state.files_db_client,
        request.id,
        new_version,
        &request.new_content,
    )
    .await
    .map_err(|err| {
        internal!(
            "Cannot create file: {}:{}:{} in S3: {:?}",
            request.id,
            request.old_metadata_version,
            new_version,
            err
        )
    })?;
    file_content_client::delete(
        &server_state.files_db_client,
        request.id,
        request.old_metadata_version,
    )
    .await
    .map_err(|err| {
        internal!(
            "Cannot delete file: {}:{}:{} in S3: {:?}",
            request.id,
            request.old_metadata_version,
            new_version,
            err
        )
    })?;

    Err(internal!(""))
}

pub async fn get_document(
    context: RequestContext<'_, GetDocumentRequest>,
) -> Result<GetDocumentResponse, ServerError<GetDocumentError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let files_result = file_content_client::get(
        &server_state.files_db_client,
        request.id,
        request.content_version,
    )
    .await;
    match files_result {
        Ok(c) => Ok(GetDocumentResponse { content: c }),
        Err(file_content_client::Error::NoSuchKey(_)) => {
            Err(ClientError(GetDocumentError::DocumentNotFound))
        }
        Err(e) => Err(InternalError(format!("Cannot get file from S3: {:?}", e))),
    }
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
    let files: Vec<EncryptedFileMetadata> = con.json_mget(keys).await?;

    let file_metadata = files
        .into_iter()
        .filter(|meta| meta.metadata_version > request.since_metadata_version)
        .collect();

    Ok(GetUpdatesResponse { file_metadata })
}
