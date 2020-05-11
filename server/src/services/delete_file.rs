use crate::files_db;
use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::{DeleteFileError, DeleteFileRequest, DeleteFileResponse};

pub fn delete_file(
    server_state: &mut ServerState,
    request: DeleteFileRequest,
) -> Result<DeleteFileResponse, DeleteFileError> {
    let index_db_delete_file_result =
        index_db::delete_file(&mut server_state.index_db_client, &request.file_id);
    match index_db_delete_file_result {
        Ok(_) => {}
        Err(index_db::delete_file::Error::FileDoesNotExist) => {
            return Err(DeleteFileError::FileNotFound)
        }
        Err(index_db::delete_file::Error::FileDeleted) => return Err(DeleteFileError::FileDeleted),
        Err(index_db::delete_file::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", index_db_delete_file_result);
            return Err(DeleteFileError::InternalError);
        }
        Err(index_db::delete_file::Error::VersionGeneration(_)) => {
            println!("Internal server error! {:?}", index_db_delete_file_result);
            return Err(DeleteFileError::InternalError);
        }
    };

    let filed_db_delete_file_result =
        files_db::delete_file(&server_state.files_db_client, &request.file_id);
    match filed_db_delete_file_result {
        Ok(()) => Ok(DeleteFileResponse {}),
        Err(_) => {
            println!("Internal server error! {:?}", filed_db_delete_file_result);
            return Err(DeleteFileError::InternalError);
        }
    }
}
