use crate::index_db;
use crate::Endpoint;
use crate::ServerState;
use lockbook_core::model::api::{NewAccountError, NewAccountRequest, NewAccountResponse};

pub struct EndpointImpl;

impl Endpoint<NewAccountRequest, NewAccountResponse, NewAccountError>
    for EndpointImpl
{
    fn handle(
        server_state: &mut ServerState,
        request: NewAccountRequest,
    ) -> Result<NewAccountResponse, NewAccountError> {
        handle(server_state, request)
    }
}

fn handle(
    server_state: &mut ServerState,
    request: NewAccountRequest,
) -> Result<NewAccountResponse, NewAccountError> {
    let new_account_result = index_db::new_account(
        &mut server_state.index_db_client,
        &request.username,
        &request.public_key,
    );
    match new_account_result {
        Ok(()) => Ok(NewAccountResponse {}),
        Err(index_db::new_account::Error::UsernameTaken) => Err(NewAccountError::UsernameTaken),
        Err(index_db::new_account::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", new_account_result);
            Err(NewAccountError::InternalError)
        }
    }
}
