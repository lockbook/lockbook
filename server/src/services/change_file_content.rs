use crate::config::ServerState;
use crate::files_db;
use crate::index_db;
use lockbook_core::client::ChangeFileContentResponse;

pub struct ChangeFileContentRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub old_file_version: i64,
    pub new_file_content: String,
}

pub enum ChangeFileContentError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    EditConflict,
    FileDeleted,
}

pub fn change_file_content(
    server: ServerState,
    request: ChangeFileContentRequest,
) -> Result<ChangeFileContentResponse, ChangeFileContentError> {
    let mut locked_index_db_client = server.index_db_client.lock().unwrap();
    let locked_files_db_client = server.files_db_client.lock().unwrap();

    let update_file_version_result = index_db::update_file_version(
        &mut locked_index_db_client,
        &request.file_id,
        &request.old_file_version,
    );
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
        &locked_files_db_client,
        &request.file_id,
        &request.new_file_content,
    );
    match create_file_result {
        Ok(()) => Ok(ChangeFileContentResponse {
            current_version: new_version as u64,
            error_code: String::default(),
        }),
        Err(_) => {
            println!("Internal server error! {:?}", create_file_result);
            return Err(ChangeFileContentError::InternalError);
        }
    }
}
