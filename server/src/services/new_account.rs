use crate::index_db;
use crate::ServerState;
use rsa::RSAPublicKey;
use lockbook_core::service::crypto_service::{SignedValue, RsaImpl, PubKeyCryptoService};
use lockbook_core::model::api::{NewAccountError, NewAccountRequest, NewAccountResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: NewAccountRequest,
) -> Result<NewAccountResponse, NewAccountError> {

    let public_key = serde_json::from_str::<RSAPublicKey>(&request.public_key).map_err(|_| NewAccountError::InvalidPublicKey)?;
    let auth = serde_json::from_str::<SignedValue>(&request.auth).map_err(|_| NewAccountError::InvalidAuth)?;
    RsaImpl::verify(&public_key, &auth).map_err(|_| NewAccountError::InvalidPublicKey)?;

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
