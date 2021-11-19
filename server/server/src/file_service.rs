use crate::file_index_repo;
use crate::file_index_repo::UpsertFileMetadataError;
use crate::file_index_repo::{CheckCyclesError, GetDataCapError};
use crate::RequestContext;
use crate::ServerError::{ClientError, InternalError};
use crate::{file_content_client, ServerError};

use libsecp256k1::PublicKey;
use lockbook_models::api::*;
use lockbook_models::file_metadata::FileType;
use sqlx::types::Uuid;
use sqlx::{Postgres, Transaction};

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

async fn has_space_for_document(
    transaction: &mut Transaction<'_, Postgres>,
    pk: &PublicKey,
    id: Uuid,
    new_content: usize,
) -> Result<bool, ServerError<ChangeDocumentContentError>> {
    let new_size: u64 = file_index_repo::get_file_usages(transaction, &pk)
        .await
        .map_err(|err| {
            InternalError(format!(
                "Could not get usages for pk: {:?} err: {:?}",
                pk, err
            ))
        })?
        .into_iter()
        .map(|usage| {
            if usage.file_id == id {
                new_content as u64
            } else {
                usage.size_bytes
            }
        })
        .sum();

    let data_cap: u64 = file_index_repo::get_account_data_cap(transaction, &pk)
        .await
        .map_err(|err| match err {
            GetDataCapError::TierNotFound => {
                ServerError::ClientError(ChangeDocumentContentError::UserNotFound)
            }
            _ => InternalError(format!(
                "Could not lookup usage for pk: {:?}, error: {:?}",
                pk, err
            )),
        })?;

    Ok(new_size < data_cap)
}

pub async fn change_document_content(
    context: RequestContext<'_, ChangeDocumentContentRequest>,
) -> Result<ChangeDocumentContentResponse, ServerError<ChangeDocumentContentError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    // Check if the person has space for this document and is an actual person in our db
    if !has_space_for_document(
        &mut transaction,
        &context.public_key,
        context.request.id,
        context.request.new_content.value.len(),
    )
    .await?
    {
        return Err(ClientError(ChangeDocumentContentError::OutOfSpace));
    }

    let result = file_index_repo::change_document_version_and_size(
        &mut transaction,
        request.id,
        request.new_content.value.len() as u64,
        request.old_metadata_version,
    )
    .await;

    let (old_content_version, new_version) = result.map_err(|e| match e {
        file_index_repo::ChangeDocumentVersionAndSizeError::DoesNotExist => {
            ClientError(ChangeDocumentContentError::DocumentNotFound)
        }
        file_index_repo::ChangeDocumentVersionAndSizeError::IncorrectOldVersion => {
            ClientError(ChangeDocumentContentError::EditConflict)
        }
        file_index_repo::ChangeDocumentVersionAndSizeError::Deleted => {
            ClientError(ChangeDocumentContentError::DocumentDeleted)
        }
        file_index_repo::ChangeDocumentVersionAndSizeError::Postgres(_)
        | file_index_repo::ChangeDocumentVersionAndSizeError::Deserialize(_) => {
            InternalError(format!(
                "Cannot change document content version in Postgres: {:?}",
                e
            ))
        }
    })?;

    let create_result = file_content_client::create(
        &server_state.files_db_client,
        request.id,
        new_version,
        &request.new_content,
    )
    .await;
    if create_result.is_err() {
        return Err(InternalError(format!(
            "Cannot create file in S3: {:?}",
            create_result
        )));
    };

    let delete_result = file_content_client::delete(
        &server_state.files_db_client,
        request.id,
        old_content_version,
    )
    .await;
    if delete_result.is_err() {
        return Err(InternalError(format!(
            "Cannot delete file in S3: {:?}",
            delete_result
        )));
    };

    match transaction.commit().await {
        Ok(()) => Ok(ChangeDocumentContentResponse {
            new_content_version: new_version,
        }),
        Err(e) => Err(InternalError(format!("Cannot commit transaction: {:?}", e))),
    }
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
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };
    let result = file_index_repo::get_updates(
        &mut transaction,
        &context.public_key,
        request.since_metadata_version,
    )
    .await;
    let updates =
        result.map_err(|e| InternalError(format!("Cannot get updates from Postgres: {:?}", e)))?;

    match transaction.commit().await {
        Ok(()) => Ok(GetUpdatesResponse {
            file_metadata: updates,
        }),
        Err(e) => Err(InternalError(format!("Cannot commit transaction: {:?}", e))),
    }
}
