use crate::config::ServerState;
use crate::files_db;
use crate::index_db;
use lockbook_core::client::DeleteFileResponse;

pub struct DeleteFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
}

pub enum DeleteFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    FileDeleted,
}

pub fn delete_file(
    server: ServerState,
    request: DeleteFileRequest,
) -> Result<DeleteFileResponse, DeleteFileError> {
    let mut locked_index_db_client = server.index_db_client.lock().unwrap();
    let locked_files_db_client = server.files_db_client.lock().unwrap();

    let index_db_delete_file_result =
        index_db::delete_file(&mut locked_index_db_client, &request.file_id);
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
        files_db::delete_file(&locked_files_db_client, &request.file_id);
    match filed_db_delete_file_result {
        Ok(()) => Ok(DeleteFileResponse {
            error_code: String::default(),
        }),
        Err(_) => {
            println!("Internal server error! {:?}", filed_db_delete_file_result);
            return Err(DeleteFileError::InternalError);
        }
    }
}
