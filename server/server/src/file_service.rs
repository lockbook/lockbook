use crate::{file_index_repo, file_content_client, RequestContext};
use lockbook_models::api::*;

pub async fn upsert_file_metadata(
    context: RequestContext<'_, FileMetadataUpsertsRequest>,
) -> Result<(), Result<FileMetadataUpsertsError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    for upsert in context.request.updates {
        let index_result = file_index_repo::upsert_file_metadata(
            &mut transaction,
            &context.public_key,
            &upsert,
        )
        .await;
        // todo: check if pull is required
    };

    // todo: create empty files for new files
    // let files_result = file_content_client::create(
    //     &server_state.files_db_client,
    //     request.id,
    //     new_version,
    //     &request.content,
    // )
    // .await;

    // if files_result.is_err() {
    //     return Err(Err(format!("Cannot create file in S3: {:?}", files_result)));
    // };

    match transaction.commit().await {
        Ok(()) => Ok(()),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn change_document_content(
    context: RequestContext<'_, ChangeDocumentContentRequest>,
) -> Result<ChangeDocumentContentResponse, Result<ChangeDocumentContentError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let result = file_index_repo::change_document_version_and_size(
        &mut transaction,
        request.id,
        request.new_content.value.len() as u64,
        request.old_metadata_version,
    )
    .await;

    let (old_content_version, new_version) = result.map_err(|e| match e {
        file_index_repo::ChangeDocumentVersionAndSizeError::DoesNotExist => {
            Ok(ChangeDocumentContentError::DocumentNotFound)
        }
        file_index_repo::ChangeDocumentVersionAndSizeError::IncorrectOldVersion => {
            Ok(ChangeDocumentContentError::EditConflict)
        }
        file_index_repo::ChangeDocumentVersionAndSizeError::Deleted => {
            Ok(ChangeDocumentContentError::DocumentDeleted)
        }
        file_index_repo::ChangeDocumentVersionAndSizeError::Postgres(_)
        | file_index_repo::ChangeDocumentVersionAndSizeError::Deserialize(_) => Err(format!(
            "Cannot change document content version in Postgres: {:?}",
            e
        )),
    })?;

    let create_result = file_content_client::create(
        &server_state.files_db_client,
        request.id,
        new_version,
        &request.new_content,
    )
    .await;
    if create_result.is_err() {
        return Err(Err(format!(
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
        return Err(Err(format!(
            "Cannot delete file in S3: {:?}",
            delete_result
        )));
    };

    match transaction.commit().await {
        Ok(()) => Ok(ChangeDocumentContentResponse {}),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn get_document(
    context: RequestContext<'_, GetDocumentRequest>,
) -> Result<GetDocumentResponse, Result<GetDocumentError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
    let files_result = file_content_client::get(
        &server_state.files_db_client,
        request.id,
        request.content_version,
    )
    .await;
    match files_result {
        Ok(c) => Ok(GetDocumentResponse { content: c }),
        Err(file_content_client::Error::NoSuchKey(_)) => Err(Ok(GetDocumentError::DocumentNotFound)),
        Err(e) => Err(Err(format!("Cannot get file from S3: {:?}", e))),
    }
}

pub async fn get_updates(
    context: RequestContext<'_, GetUpdatesRequest>,
) -> Result<GetUpdatesResponse, Result<GetUpdatesError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };
    let result = file_index_repo::get_updates(
        &mut transaction,
        &context.public_key,
        request.since_metadata_version,
    )
    .await;
    let updates = result.map_err(|e| Err(format!("Cannot get updates from Postgres: {:?}", e)))?;

    match transaction.commit().await {
        Ok(()) => Ok(GetUpdatesResponse {
            file_metadata: updates,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}
