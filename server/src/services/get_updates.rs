use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::{GetUpdatesError, GetUpdatesRequest, GetUpdatesResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: GetUpdatesRequest,
) -> Result<GetUpdatesResponse, GetUpdatesError> {
    if !request.username.chars().all(|x| x.is_digit(36)) {
        return Err(GetUpdatesError::InvalidUsername);
    }
    let get_updates_result = index_db::get_updates(
        &mut server_state.index_db_client,
        &request.username,
        request.since_version as i64,
    )
    .await;
    match get_updates_result {
        Ok(updates) => Ok(GetUpdatesResponse {
            file_metadata: updates,
        }),
        Err(_) => {
            println!("Internal server error! {:?}", get_updates_result);
            Err(GetUpdatesError::InternalError)
        }
    }
}
