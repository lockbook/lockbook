use crate::index_db;
use crate::services::username_is_valid;
use crate::ServerState;
use lockbook_core::model::api::{RenameFileError, RenameFileRequest, RenameFileResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: RenameFileRequest,
) -> Result<RenameFileResponse, RenameFileError> {
    if !username_is_valid(&request.username) {
        return Err(RenameFileError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(RenameFileError::InternalError);
        }
    };

    let rename_file_result =
        index_db::rename_file(&transaction, &request.file_id, &request.new_file_name).await;
    let result = match rename_file_result {
        Ok(_) => Ok(RenameFileResponse {}),
        Err(index_db::rename_file::Error::FileDoesNotExist) => Err(RenameFileError::FileNotFound),
        Err(index_db::rename_file::Error::FileDeleted) => Err(RenameFileError::FileDeleted),
        Err(index_db::rename_file::Error::Uninterpreted(_)) => {
            error!("Internal server error! {:?}", rename_file_result);
            Err(RenameFileError::InternalError)
        }
        Err(index_db::rename_file::Error::VersionGeneration(_)) => {
            error!("Internal server error! {:?}", rename_file_result);
            Err(RenameFileError::InternalError)
        }
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(RenameFileError::InternalError)
        }
    }
}
