use crate::files_db;
use crate::index_db::{update_file_metadata_and_content_version, update_file_metadata_version};
use crate::services::username_is_valid;
use crate::ServerState;
use lockbook_core::model::api::{
    ChangeDocumentContentError, ChangeFileContentRequest, ChangeFileContentResponse,
};

pub async fn handle(
    server_state: &mut ServerState,
    request: ChangeFileContentRequest,
) -> Result<ChangeFileContentResponse, ChangeDocumentContentError> {
    if !username_is_valid(&request.username) {
        return Err(ChangeDocumentContentError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(ChangeDocumentContentError::InternalError);
        }
    };

    let update_file_version_result = update_file_metadata_and_content_version(
        &transaction,
        &request.file_id,
        request.old_metadata_version as u64,
    )
    .await;
    let (old_content_version, new_version) = match update_file_version_result {
        Ok(x) => x,
        Err(update_file_metadata_and_content_version::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeDocumentContentError::InternalError);
        }
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::Uninterpreted(_),
        )) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeDocumentContentError::InternalError);
        }
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::VersionGeneration(_),
        )) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeDocumentContentError::InternalError);
        }
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::FileDoesNotExist,
        )) => return Err(ChangeDocumentContentError::FileNotFound),
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::IncorrectOldVersion(_),
        )) => return Err(ChangeDocumentContentError::EditConflict),
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::FileDeleted,
        )) => return Err(ChangeDocumentContentError::FileDeleted),
    };

    let create_file_result = files_db::create_file(
        &server_state.files_db_client,
        &request.file_id,
        &request.new_file_content,
        new_version,
    )
    .await;
    if create_file_result.is_err() {
        println!("Internal server error! {:?}", create_file_result);
        return Err(ChangeDocumentContentError::InternalError);
    };

    let delete_file_result = files_db::delete_file(
        &server_state.files_db_client,
        &request.file_id,
        old_content_version,
    )
    .await;
    if delete_file_result.is_err() {
        println!("Internal server error! {:?}", delete_file_result);
        return Err(ChangeDocumentContentError::InternalError);
    };

    match transaction.commit().await {
        Ok(_) => Ok(ChangeFileContentResponse {
            current_metadata_and_content_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(ChangeDocumentContentError::InternalError)
        }
    }
}
