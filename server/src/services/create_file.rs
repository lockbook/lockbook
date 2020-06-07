use crate::files_db;
use crate::index_db;
use crate::services::username_is_valid;
use crate::ServerState;
use lockbook_core::model::api::{CreateFileError, CreateFileRequest, CreateFileResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: CreateFileRequest,
) -> Result<CreateFileResponse, CreateFileError> {
    if !username_is_valid(&request.username) {
        return Err(CreateFileError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(CreateFileError::InternalError);
        }
    };
    let get_file_details_result =
        files_db::get_file_details(&server_state.files_db_client, &request.file_id).await;
    match get_file_details_result {
        Err(files_db::get_file_details::Error::NoSuchFile(())) => {}
        Err(_) => {
            error!("Internal server error! {:?}", get_file_details_result);
            return Err(CreateFileError::InternalError);
        }
        Ok(_) => return Err(CreateFileError::FileIdTaken),
    };

    let index_db_create_file_result = index_db::create_file(
        &transaction,
        &request.file_id,
        &request.username,
        &request.file_name,
        &request.file_path,
    )
    .await;
    let new_version = match index_db_create_file_result {
        Ok(version) => version,
        Err(index_db::create_file::Error::FileIdTaken) => return Err(CreateFileError::FileIdTaken),
        Err(index_db::create_file::Error::FilePathTaken) => {
            return Err(CreateFileError::FilePathTaken)
        }
        Err(index_db::create_file::Error::Uninterpreted(_)) => {
            error!("Internal server error! {:?}", index_db_create_file_result);
            return Err(CreateFileError::InternalError);
        }
        Err(index_db::create_file::Error::VersionGeneration(_)) => {
            error!("Internal server error! {:?}", index_db_create_file_result);
            return Err(CreateFileError::InternalError);
        }
    } as u64;

    let files_db_create_file_result = files_db::create_file(
        &server_state.files_db_client,
        &request.file_id,
        &request.file_content,
        new_version,
    )
    .await;
    if files_db_create_file_result.is_err() {
        println!("Internal server error! {:?}", files_db_create_file_result);
        return Err(CreateFileError::InternalError);
    };

    match transaction.commit().await {
        Ok(_) => Ok(CreateFileResponse {
            current_metadata_and_content_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(CreateFileError::InternalError)
        }
    }
}
