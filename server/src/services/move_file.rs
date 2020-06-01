use crate::index_db;
use crate::services::username_is_valid;
use crate::ServerState;
use lockbook_core::model::api::{MoveFileError, MoveFileRequest, MoveFileResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: MoveFileRequest,
) -> Result<MoveFileResponse, MoveFileError> {
    if !username_is_valid(&request.username) {
        return Err(MoveFileError::InvalidUsername);
    }
    let move_file_result = index_db::move_file(
        &mut server_state.index_db_client,
        &request.file_id,
        &request.new_file_path,
    )
    .await;
    match move_file_result {
        Ok(_) => Ok(MoveFileResponse {}),
        Err(index_db::move_file::Error::FileDoesNotExist) => Err(MoveFileError::FileNotFound),
        Err(index_db::move_file::Error::FileDeleted) => Err(MoveFileError::FileDeleted),
        Err(index_db::move_file::Error::FilePathTaken) => Err(MoveFileError::FilePathTaken),
        Err(index_db::move_file::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", move_file_result);
            Err(MoveFileError::InternalError)
        }
        Err(index_db::move_file::Error::VersionGeneration(_)) => {
            println!("Internal server error! {:?}", move_file_result);
            Err(MoveFileError::InternalError)
        }
    }
}
