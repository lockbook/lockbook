use crate::index_db;
use crate::services::username_is_valid;
use crate::ServerState;
use lockbook_core::model::api::{GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: GetPublicKeyRequest,
) -> Result<GetPublicKeyResponse, GetPublicKeyError> {
    if !username_is_valid(&request.username) {
        return Err(GetPublicKeyError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(GetPublicKeyError::InternalError);
        }
    };
    let get_public_key_result = index_db::get_public_key(&transaction, &request.username).await;
    let result = match get_public_key_result {
        Ok(key) => Ok(GetPublicKeyResponse { key: key }),
        Err(index_db::get_public_key::Error::Postgres(_)) => Err(GetPublicKeyError::UserNotFound),
        Err(index_db::get_public_key::Error::SerializationError(_)) => {
            error!("Internal server error! {:?}", get_public_key_result);
            Err(GetPublicKeyError::InternalError)
        }
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(GetPublicKeyError::InternalError)
        }
    }
}
