use crate::config::ServerState;
use crate::index_db;
use lockbook_core::client::FileMetadata;

pub struct GetUpdatesResponse {
    pub updated_metadata: Vec<FileMetadata>,
}

pub struct GetUpdatesRequest {
    pub username: String,
    pub auth: String,
    pub version: i64,
}

pub enum GetUpdatesError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
}

pub fn get_updates(
    server: ServerState,
    request: GetUpdatesRequest,
) -> Result<GetUpdatesResponse, GetUpdatesError> {
    let mut locked_index_db_client = server.index_db_client.lock().unwrap();

    let get_updates_result = index_db::get_updates(
        &mut locked_index_db_client,
        &request.username,
        &request.version,
    );
    match get_updates_result {
        Ok(updates) => Ok(GetUpdatesResponse {
            updated_metadata: updates,
        }),
        Err(_) => {
            println!("Internal server error! {:?}", get_updates_result);
            Err(GetUpdatesError::InternalError)
        }
    }
}
