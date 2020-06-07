use crate::index_db;
use crate::services::username_is_valid;
use crate::ServerState;
use lockbook_core::model::api::{NewAccountError, NewAccountRequest, NewAccountResponse};
use lockbook_core::service::crypto_service::{PubKeyCryptoService, RsaImpl, SignedValue};

pub async fn handle(
    server_state: &mut ServerState,
    request: NewAccountRequest,
) -> Result<NewAccountResponse, NewAccountError> {
    let auth = serde_json::from_str::<SignedValue>(&request.auth)
        .map_err(|_| NewAccountError::InvalidAuth)?;
    RsaImpl::verify(&request.public_key, &auth).map_err(|_| NewAccountError::InvalidPublicKey)?;
    if !username_is_valid(&request.username) {
        return Err(NewAccountError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(NewAccountError::InternalError);
        }
    };

    let new_account_result =
        index_db::new_account(&transaction, &request.username, &serde_json::to_string(&request.public_key).map_err(|_| NewAccountError::InvalidPublicKey)?).await;
    let result = match new_account_result {
        Ok(()) => Ok(NewAccountResponse {}),
        Err(index_db::new_account::Error::UsernameTaken) => Err(NewAccountError::UsernameTaken),
        Err(index_db::new_account::Error::Uninterpreted(_)) => {
            error!("Internal server error! {:?}", new_account_result);
            Err(NewAccountError::InternalError)
        }
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(NewAccountError::InternalError)
        }
    }
}
