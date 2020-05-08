use crate::config::ServerState;
use crate::files_db;
use crate::index_db;
use lockbook_core::client::CreateFileResponse;

pub struct CreateFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content: String,
}

pub enum CreateFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileIdTaken,
    FilePathTaken,
}

pub fn create_file(
    server: ServerState,
    request: CreateFileRequest,
) -> Result<CreateFileResponse, CreateFileError> {
    let mut locked_index_db_client = server.index_db_client.lock().unwrap();
    let locked_files_db_client = server.files_db_client.lock().unwrap();

    let get_file_details_result =
        files_db::get_file_details(&locked_files_db_client, &request.file_id);
    match get_file_details_result {
        Err(files_db::get_file_details::Error::NoSuchFile(())) => {}
        Err(_) => {
            println!("Internal server error! {:?}", get_file_details_result);
            return Err(CreateFileError::InternalError);
        }
        Ok(_) => return Err(CreateFileError::FileIdTaken),
    };

    let index_db_create_file_result = index_db::create_file(
        &mut locked_index_db_client,
        &request.file_id,
        &request.username,
        &request.file_name,
        &request.file_path,
    );
    let new_version = match index_db_create_file_result {
        Ok(version) => version,
        Err(index_db::create_file::Error::FileIdTaken) => return Err(CreateFileError::FileIdTaken),
        Err(index_db::create_file::Error::FilePathTaken) => {
            return Err(CreateFileError::FilePathTaken)
        }
        Err(index_db::create_file::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", index_db_create_file_result);
            return Err(CreateFileError::InternalError);
        }
        Err(index_db::create_file::Error::VersionGeneration(_)) => {
            println!("Internal server error! {:?}", index_db_create_file_result);
            return Err(CreateFileError::InternalError);
        }
    };

    let files_db_create_file_result = files_db::create_file(
        &locked_files_db_client,
        &request.file_id,
        &request.file_content,
    );
    match files_db_create_file_result {
        Ok(()) => Ok(CreateFileResponse {
            current_version: new_version as u64,
            error_code: String::default(),
        }),
        Err(_) => {
            println!("Internal server error! {:?}", files_db_create_file_result);
            Err(CreateFileError::InternalError)
        }
    }
}
