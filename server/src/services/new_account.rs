use crate::index_db;
use crate::services::username_is_valid;
use crate::ServerState;
use lockbook_core::model::api::{NewAccountError, NewAccountRequest, NewAccountResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: NewAccountRequest,
) -> Result<NewAccountResponse, NewAccountError> {
    if !username_is_valid(&request.username) {
        return Err(NewAccountError::InvalidUsername);
    }
    let new_account_result = index_db::new_account(
        &mut server_state.index_db_client,
        &request.username,
        &request.public_key,
    )
    .await;
    match new_account_result {
        Ok(()) => Ok(NewAccountResponse {}),
        Err(index_db::new_account::Error::UsernameTaken) => Err(NewAccountError::UsernameTaken),
        Err(index_db::new_account::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", new_account_result);
            Err(NewAccountError::InternalError)
        }
    }
}
