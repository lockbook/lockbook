use crate::config::ServerState;
use crate::index_db;
use lockbook_core::client::RenameFileResponse;

pub struct RenameFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub new_file_name: String,
}

pub enum RenameFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    FileDeleted,
}

pub fn rename_file(
    server: ServerState,
    request: RenameFileRequest,
) -> Result<RenameFileResponse, RenameFileError> {
    let mut locked_index_db_client = server.index_db_client.lock().unwrap();

    let rename_file_result = index_db::rename_file(
        &mut locked_index_db_client,
        &request.file_id,
        &request.new_file_name,
    );
    match rename_file_result {
        Ok(_) => Ok(RenameFileResponse {
            error_code: String::default(),
        }),
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
