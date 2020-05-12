use crate::index_db;
use crate::Endpoint;
use crate::ServerState;
use lockbook_core::model::api::{GetUpdatesError, GetUpdatesRequest, GetUpdatesResponse};

pub struct EndpointImpl;

impl Endpoint<GetUpdatesRequest, GetUpdatesResponse, GetUpdatesError> for EndpointImpl {
    fn handle(
        server_state: &mut ServerState,
        request: GetUpdatesRequest,
    ) -> Result<GetUpdatesResponse, GetUpdatesError> {
        handle(server_state, request)
    }
}

fn handle(
    server_state: &mut ServerState,
    request: GetUpdatesRequest,
) -> Result<GetUpdatesResponse, GetUpdatesError> {
    let get_updates_result = index_db::get_updates(
        &mut server_state.index_db_client,
        &request.username,
        &(request.since_version as i64),
    );
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
