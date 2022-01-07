use crate::file_index_repo::CheckCyclesError;
use crate::file_index_repo::UpsertFileMetadataError;
use crate::keys::{file, owned_files, size};
use crate::RequestContext;
use crate::ServerError::{ClientError, InternalError};
use crate::{file_content_client, ServerError};
use crate::{file_index_repo, keys};
use lockbook_crypto::clock_service::get_time;
use lockbook_models::api::*;
use lockbook_models::file_metadata::{EncryptedFileMetadata, FileType};
use redis_utils::converters::{JsonGet, JsonSet};
use redis_utils::tx;
use uuid::Uuid;

pub async fn upsert_file_metadata(
    context: RequestContext<'_, FileMetadataUpsertsRequest>,
) -> Result<(), ServerError<FileMetadataUpsertsError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    for upsert in &request.updates {
        if let Some((old_parent, _)) = upsert.old_parent_and_name {
            // prevent all updates to root
            if upsert.id == old_parent {
                return Err(ClientError(FileMetadataUpsertsError::GetUpdatesRequired));
                // todo: better error
            }
            // prevent turning existing folder into root
            if upsert.id == upsert.new_parent {
                return Err(ClientError(FileMetadataUpsertsError::GetUpdatesRequired));
                // todo: better error
            }
        }

        let index_result =
            file_index_repo::upsert_file_metadata(&mut transaction, &context.public_key, upsert)
                .await;
        match index_result {
            Ok(new_version) => {
                // create empty files for new document
                if upsert.old_parent_and_name.is_none() && upsert.file_type == FileType::Document {
                    let files_result = file_content_client::create_empty(
                        &server_state.files_db_client,
                        upsert.id,
                        new_version,
                    )
                    .await;
                    if let Err(e) = files_result {
                        return Err(InternalError(format!("Cannot create file in S3: {:?}", e)));
                    }
                }
            }
            Err(UpsertFileMetadataError::FailedPreconditions) => {
                return Err(ClientError(FileMetadataUpsertsError::GetUpdatesRequired))
            }
            Err(e) => {
                return Err(InternalError(format!("Cannot upsert metadata: {:?}", e)));
            }
        }
    }

    let cycles_result = file_index_repo::check_cycles(&mut transaction, &context.public_key).await;
    match cycles_result {
        Ok(()) => {}
        Err(CheckCyclesError::CyclesDetected) => {
            return Err(ClientError(FileMetadataUpsertsError::GetUpdatesRequired))
        }
        Err(e) => {
            return Err(InternalError(format!("Cannot check cycles: {:?}", e)));
        }
    }

    let deletions_result =
        file_index_repo::apply_recursive_deletions(&mut transaction, &context.public_key).await;
    let deleted_file_ids = match deletions_result {
        Ok(ids) => ids,
        Err(e) => {
            return Err(InternalError(format!(
                "Cannot apply recursive deletions: {:?}",
                e
            )));
        }
    };

    match transaction.commit().await {
        Ok(()) => Ok(()),
        Err(sqlx::Error::Database(db_err)) => match db_err.constraint() {
            Some("uk_files_name_parent") => {
                Err(ClientError(FileMetadataUpsertsError::GetUpdatesRequired))
            }
            Some("fk_files_parent_files_id") => {
                Err(ClientError(FileMetadataUpsertsError::GetUpdatesRequired))
            }
            _ => Err(InternalError(format!(
                "Cannot commit transaction due to constraint violation: {:?}",
                db_err
            ))),
        },
        Err(e) => Err(InternalError(format!("Cannot commit transaction: {:?}", e))),
    }?;

    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let files = file_index_repo::get_files(&mut transaction, &context.public_key)
        .await
        .map_err(|e| InternalError(format!("Cannot get files: {:?}", e)))?;
    for deleted_id in request
        .updates
        .iter()
        .filter(|upsert| upsert.new_deleted)
        .map(|upsert| upsert.id)
        .chain(deleted_file_ids.into_iter())
    {
        let content_version = files
            .iter()
            .find(|&f| f.id == deleted_id)
            .map_or(0, |f| f.content_version);
        let delete_result =
            file_content_client::delete(&server_state.files_db_client, deleted_id, content_version)
                .await;
        if delete_result.is_err() {
            return Err(InternalError(format!(
                "Cannot delete file in S3: {:?}",
                delete_result
            )));
        };
    }

    match transaction.commit().await {
        Ok(()) => Ok(()),
        Err(e) => Err(InternalError(format!("Cannot commit transaction: {:?}", e))),
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
    let mut con = server_state.index_db2_connection.get().await?;

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
    let mut con = context.server_state.index_db2_connection.get().await?;
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
