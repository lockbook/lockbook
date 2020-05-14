use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::{GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse};

pub async fn handle(
    server_state: &mut ServerState,
    request: GetPublicKeyRequest,
) -> Result<GetPublicKeyResponse, GetPublicKeyError> {
    let get_public_key_result =
        index_db::get_public_key(&mut server_state.index_db_client, &request.username).await;
    match get_public_key_result {
        Ok(key) => Ok(GetPublicKeyResponse { key: key }),
        Err(index_db::get_public_key::Error::Postgres(_)) => Err(GetPublicKeyError::UserNotFound),
        Err(index_db::get_public_key::Error::SerializationError(_)) => {
            println!("Internal server error! {:?}", get_public_key_result);
            Err(GetPublicKeyError::InternalError)
        }
    }
}
