use crate::files_db;
use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::{
    ChangeFileContentError, ChangeFileContentRequest, ChangeFileContentResponse,
};

pub async fn handle(
    server_state: &mut ServerState,
    request: ChangeFileContentRequest,
) -> Result<ChangeFileContentResponse, ChangeFileContentError> {
    let update_file_version_result = index_db::update_file_version(
        &mut server_state.index_db_client,
        &request.file_id,
        &(request.old_file_version as i64),
    )
    .await;
    let new_version = match update_file_version_result {
        Ok(new_version) => new_version,
        Err(index_db::update_file_version::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeFileContentError::InternalError);
        }
        Err(index_db::update_file_version::Error::VersionGeneration(_)) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeFileContentError::InternalError);
        }
        Err(index_db::update_file_version::Error::FileDoesNotExist) => {
            return Err(ChangeFileContentError::FileNotFound)
        }
        Err(index_db::update_file_version::Error::IncorrectOldVersion(_)) => {
            return Err(ChangeFileContentError::EditConflict)
        }
        Err(index_db::update_file_version::Error::FileDeleted) => {
            return Err(ChangeFileContentError::FileDeleted)
        }
    };

    let create_file_result = files_db::create_file(
        &server_state.files_db_client,
        &request.file_id,
        &request.new_file_content,
    )
    .await;
    match create_file_result {
        Ok(()) => Ok(ChangeFileContentResponse {
            current_version: new_version as u64,
        }),
        Err(_) => {
            println!("Internal server error! {:?}", create_file_result);
            return Err(ChangeFileContentError::InternalError);
        }
    }
}
