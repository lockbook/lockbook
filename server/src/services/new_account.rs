use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::{NewAccountError, NewAccountRequest, NewAccountResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: NewAccountRequest,
) -> Result<NewAccountResponse, NewAccountError> {
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            println!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(NewAccountError::InternalError);
        }
    };

    let new_account_result =
        index_db::new_account(&transaction, &request.username, &request.public_key).await;
    let result = match new_account_result {
        Ok(()) => Ok(NewAccountResponse {}),
        Err(index_db::new_account::Error::UsernameTaken) => Err(NewAccountError::UsernameTaken),
        Err(index_db::new_account::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", new_account_result);
            Err(NewAccountError::InternalError)
        }
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            println!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(NewAccountError::InternalError)
        }
    }
}
