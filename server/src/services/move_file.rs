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
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(MoveFileError::InternalError);
        }
    };

    let move_file_result =
        index_db::move_file(&transaction, &request.file_id, &request.new_file_path).await;
    let result = match move_file_result {
        Ok(_) => Ok(MoveFileResponse {}),
        Err(index_db::move_file::Error::FileDoesNotExist) => Err(MoveFileError::FileNotFound),
        Err(index_db::move_file::Error::FileDeleted) => Err(MoveFileError::FileDeleted),
        Err(index_db::move_file::Error::FilePathTaken) => Err(MoveFileError::FilePathTaken),
        Err(index_db::move_file::Error::Uninterpreted(_)) => {
            error!("Internal server error! {:?}", move_file_result);
            Err(MoveFileError::InternalError)
        }
        Err(index_db::move_file::Error::VersionGeneration(_)) => {
            error!("Internal server error! {:?}", move_file_result);
            Err(MoveFileError::InternalError)
        }
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(MoveFileError::InternalError)
        }
    }
}
