use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::{MoveFileError, MoveFileRequest, MoveFileResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: MoveFileRequest,
) -> Result<MoveFileResponse, MoveFileError> {
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            println!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(MoveFileError::InternalError);
        }
    };

    let move_file_result = index_db::move_file(
        &transaction,
        &request.file_id,
        request.old_metadata_version,
        &request.new_file_path,
    )
    .await;
    let result = match move_file_result {
        Ok(v) => Ok(MoveFileResponse {
            current_metadata_version: v,
        }),
        Err(index_db::move_file::Error::FileDoesNotExist) => Err(MoveFileError::FileNotFound),
        Err(index_db::move_file::Error::FileDeleted) => Err(MoveFileError::FileDeleted),
        Err(index_db::move_file::Error::FilePathTaken) => Err(MoveFileError::FilePathTaken),
        Err(index_db::move_file::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", move_file_result);
            Err(MoveFileError::InternalError)
        }
        Err(index_db::move_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::Uninterpreted(_),
        )) => {
            println!("Internal server error! {:?}", move_file_result);
            return Err(MoveFileError::InternalError);
        }
        Err(index_db::move_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::VersionGeneration(_),
        )) => {
            println!("Internal server error! {:?}", move_file_result);
            return Err(MoveFileError::InternalError);
        }
        Err(index_db::move_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::FileDoesNotExist,
        )) => return Err(MoveFileError::FileNotFound),
        Err(index_db::move_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::IncorrectOldVersion(_),
        )) => return Err(MoveFileError::EditConflict),
        Err(index_db::move_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::FileDeleted,
        )) => return Err(MoveFileError::FileDeleted),
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            println!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(MoveFileError::InternalError)
        }
    }
}
