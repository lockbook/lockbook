use crate::index_db;
use crate::services::username_is_valid;
use crate::ServerState;
use lockbook_core::model::api::{GetUpdatesError, GetUpdatesRequest, GetUpdatesResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: GetUpdatesRequest,
) -> Result<GetUpdatesResponse, GetUpdatesError> {
    if !username_is_valid(&request.username) {
        return Err(GetUpdatesError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(GetUpdatesError::InternalError);
        }
    };
    let get_updates_result = index_db::get_updates(
        &transaction,
        &request.username,
        request.since_version as i64,
    )
    .await;
    let result = match get_updates_result {
        Ok(updates) => Ok(GetUpdatesResponse {
            file_metadata: updates,
        }),
        Err(_) => {
            error!("Internal server error! {:?}", get_updates_result);
            Err(GetUpdatesError::InternalError)
        }
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(GetUpdatesError::InternalError)
        }
    }
}
