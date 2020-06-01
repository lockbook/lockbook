use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::{RenameFileError, RenameFileRequest, RenameFileResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: RenameFileRequest,
) -> Result<RenameFileResponse, RenameFileError> {
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            println!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(RenameFileError::InternalError);
        }
    };

    let rename_file_result = index_db::rename_file(
        &transaction,
        &request.file_id,
        request.old_metadata_version,
        &request.new_file_name,
    )
    .await;
    let result = match rename_file_result {
        Ok(v) => Ok(RenameFileResponse {
            current_metadata_version: v,
        }),
        Err(index_db::rename_file::Error::FileDoesNotExist) => Err(RenameFileError::FileNotFound),
        Err(index_db::rename_file::Error::FileDeleted) => Err(RenameFileError::FileDeleted),
        Err(index_db::rename_file::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", rename_file_result);
            Err(RenameFileError::InternalError)
        }
        Err(index_db::rename_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::Uninterpreted(_),
        )) => {
            println!("Internal server error! {:?}", rename_file_result);
            return Err(RenameFileError::InternalError);
        }
        Err(index_db::rename_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::VersionGeneration(_),
        )) => {
            println!("Internal server error! {:?}", rename_file_result);
            return Err(RenameFileError::InternalError);
        }
        Err(index_db::rename_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::FileDoesNotExist,
        )) => return Err(RenameFileError::FileNotFound),
        Err(index_db::rename_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::IncorrectOldVersion(_),
        )) => return Err(RenameFileError::EditConflict),
        Err(index_db::rename_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::FileDeleted,
        )) => return Err(RenameFileError::FileDeleted),
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            println!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(RenameFileError::InternalError)
        }
    }
}
