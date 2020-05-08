use crate::config::ServerState;
use crate::index_db;
use lockbook_core::client::NewAccountResponse;

pub struct NewAccountRequest {
    pub username: String,
    pub auth: String,
    pub public_key: String,
}

pub enum NewAccountError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    UsernameTaken,
}

pub fn new_account(
    server: ServerState,
    request: NewAccountRequest,
) -> Result<NewAccountResponse, NewAccountError> {
    let mut locked_index_db_client = server.index_db_client.lock().unwrap();

    let new_account_result = index_db::new_account(
        &mut locked_index_db_client,
        &request.username,
        &request.public_key,
    );
    match new_account_result {
        Ok(()) => Ok(NewAccountResponse {
            error_code: String::default(),
        }),
        Err(index_db::new_account::Error::UsernameTaken) => Err(NewAccountError::UsernameTaken),
        Err(index_db::new_account::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", new_account_result);
            Err(NewAccountError::InternalError)
        }
    }
}
