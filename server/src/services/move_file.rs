use crate::config::ServerState;
use crate::index_db;
use lockbook_core::client::MoveFileResponse;

pub struct MoveFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub new_file_path: String,
}

pub enum MoveFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    FileDeleted,
    FilePathTaken,
}

pub fn move_file(
    server: ServerState,
    request: MoveFileRequest,
) -> Result<MoveFileResponse, MoveFileError> {
    let mut locked_index_db_client = server.index_db_client.lock().unwrap();

    let move_file_result = index_db::move_file(
        &mut locked_index_db_client,
        &request.file_id,
        &request.new_file_path,
    );
    match move_file_result {
        Ok(_) => Ok(MoveFileResponse {
            error_code: String::default(),
        }),
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
