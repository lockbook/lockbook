use redis_utils::{tx, TxError};
use crate::utils::username_is_valid;
use crate::{file_index_repo, pipe, RequestContext, ServerError};
use deadpool_redis::redis::{
    AsyncCommands, FromRedisValue, Pipeline, RedisError, RedisResult, Value,
};

use redis_utils::TxError::{Abort, DbError};
use crate::ServerError::{ClientError, InternalError};
use lockbook_models::api::NewAccountError::UsernameTaken;
use lockbook_models::api::{
    GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse, GetUsageError, GetUsageRequest,
    GetUsageResponse, NewAccountError, NewAccountRequest, NewAccountResponse,
};

pub async fn new_account(
    context: RequestContext<'_, NewAccountRequest>,
) -> Result<NewAccountResponse, ServerError<NewAccountError>> {
    let (request, server_state) = (&context.request, context.server_state);
    if !username_is_valid(&request.username) {
        return Err(ClientError(NewAccountError::InvalidUsername));
    }
    let mut con = server_state.index_db2_connection.get().await.unwrap();
    let pk = serde_json::to_string(&request.public_key)
        .map_err(|e| internal!("Could not serialize public key: {}", e))?;
    let pk_key = &format!("account:{}:public_key", request.username);

    let success: Result<(), _> = tx!(&mut con, pipe_name, &[pk_key], {
        if con.exists("&pk_key.clone()").await.unwrap() {
            Err(Abort(ClientError(UsernameTaken)))
        } else {
            Ok(pipe_name.set(pk_key, &pk))
        }
    });
    Err(internal!(""))
}

pub async fn get_public_key(
    context: RequestContext<'_, GetPublicKeyRequest>,
) -> Result<GetPublicKeyResponse, ServerError<GetPublicKeyError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };
    let result = file_index_repo::get_public_key(&mut transaction, &request.username).await;
    let key = result.map_err(|e| match e {
        file_index_repo::PublicKeyError::UserNotFound => {
            ClientError(GetPublicKeyError::UserNotFound)
        }
        _ => InternalError(format!("Cannot get public key from Postgres: {:?}", e)),
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(GetPublicKeyResponse { key: key }),
        Err(e) => Err(InternalError(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn get_usage(
    context: RequestContext<'_, GetUsageRequest>,
) -> Result<GetUsageResponse, ServerError<GetUsageError>> {
    let (_, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:#?}", e)));
        }
    };

    let usages = file_index_repo::get_file_usages(&mut transaction, &context.public_key)
        .await
        .map_err(|e| InternalError(format!("Usage calculation error: {:#?}", e)))?;

    let cap = file_index_repo::get_account_data_cap(&mut transaction, &context.public_key)
        .await
        .map_err(|e| InternalError(format!("Data cap calculation error: {:#?}", e)))?;

    Ok(GetUsageResponse { usages, cap })
}
