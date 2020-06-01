use crate::files_db;
use crate::index_db::{update_file_metadata_and_content_version, update_file_metadata_version};
use crate::ServerState;
use lockbook_core::model::api::{
    ChangeFileContentError, ChangeFileContentRequest, ChangeFileContentResponse,
};

pub async fn handle(
    server_state: &mut ServerState,
    request: ChangeFileContentRequest,
) -> Result<ChangeFileContentResponse, ChangeFileContentError> {
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            println!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(ChangeFileContentError::InternalError);
        }
    };

    let update_file_version_result = update_file_metadata_and_content_version(
        &transaction,
        &request.file_id,
        request.old_metadata_version as u64,
    )
    .await;
    let new_version = match update_file_version_result {
        Ok(new_version) => new_version,
        Err(update_file_metadata_and_content_version::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeFileContentError::InternalError);
        }
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::Uninterpreted(_),
        )) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeFileContentError::InternalError);
        }
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::VersionGeneration(_),
        )) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeFileContentError::InternalError);
        }
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::FileDoesNotExist,
        )) => return Err(ChangeFileContentError::FileNotFound),
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::IncorrectOldVersion(_),
        )) => return Err(ChangeFileContentError::EditConflict),
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::FileDeleted,
        )) => return Err(ChangeFileContentError::FileDeleted),
    };

    let create_file_result = files_db::create_file(
        &server_state.files_db_client,
        &request.file_id,
        &request.new_file_content,
    )
    .await;
    let result = match create_file_result {
        Ok(()) => Ok(ChangeFileContentResponse {
            current_metadata_and_content_version: new_version as u64,
        }),
        Err(_) => {
            println!("Internal server error! {:?}", create_file_result);
            Err(ChangeFileContentError::InternalError)
        }
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            println!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(ChangeFileContentError::InternalError)
        }
    }
}
