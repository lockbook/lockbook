use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::{RenameFileError, RenameFileRequest, RenameFileResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: RenameFileRequest,
) -> Result<RenameFileResponse, RenameFileError> {
    let rename_file_result = index_db::rename_file(
        &mut server_state.index_db_client,
        &request.file_id,
        &request.new_file_name,
    )
    .await;
    match rename_file_result {
        Ok(_) => Ok(RenameFileResponse {}),
        Err(index_db::rename_file::Error::FileDoesNotExist) => Err(RenameFileError::FileNotFound),
        Err(index_db::rename_file::Error::FileDeleted) => Err(RenameFileError::FileDeleted),
        Err(index_db::rename_file::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", rename_file_result);
            Err(RenameFileError::InternalError)
        }
        Err(index_db::rename_file::Error::VersionGeneration(_)) => {
            println!("Internal server error! {:?}", rename_file_result);
            Err(RenameFileError::InternalError)
        }
    }
}
