use crate::files_db;
use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::{CreateFileError, CreateFileRequest, CreateFileResponse};

pub struct Service;

impl crate::endpoint::EndpointService<CreateFileRequest, CreateFileResponse, CreateFileError>
    for Service
{
    fn handle(
        server_state: &mut ServerState,
        request: CreateFileRequest,
    ) -> Result<CreateFileResponse, CreateFileError> {
        handle(server_state, request)
    }
}

fn handle(
    server_state: &mut ServerState,
    request: CreateFileRequest,
) -> Result<CreateFileResponse, CreateFileError> {
    let get_file_details_result =
        files_db::get_file_details(&server_state.files_db_client, &request.file_id);
    match get_file_details_result {
        Err(files_db::get_file_details::Error::NoSuchFile(())) => {}
        Err(_) => {
            println!("Internal server error! {:?}", get_file_details_result);
            return Err(CreateFileError::InternalError);
        }
        Ok(_) => return Err(CreateFileError::FileIdTaken),
    };

    let index_db_create_file_result = index_db::create_file(
        &mut server_state.index_db_client,
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
        &server_state.files_db_client,
        &request.file_id,
        &request.file_content,
    );
    match files_db_create_file_result {
        Ok(()) => Ok(CreateFileResponse {
            current_version: new_version as u64,
        }),
        Err(_) => {
            println!("Internal server error! {:?}", files_db_create_file_result);
            Err(CreateFileError::InternalError)
        }
    }
}
