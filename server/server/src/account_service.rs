use crate::utils::username_is_valid;
use crate::{file_index_repo, pipe, RequestContext, ServerError};
use deadpool_redis::redis::{AsyncCommands, Pipeline};
use lockbook_crypto::clock_service::get_time;
use redis_utils::converters::{JsonGet, JsonSet};

use redis_utils::{tx, TxError};

use crate::keys::{file, meta, owned_files, public_key};
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
    let mut con = server_state.index_db2_connection.get().await?;

    let mut root = request.root_folder.clone();
    let now = get_time().0 as u64;
    root.metadata_version = now;

    let watched_keys = &[
        public_key(&request.username),
        owned_files(&request.username),
        file(request.root_folder.id),
    ];

    let tx_result = tx!(&mut con, pipe_name, watched_keys, {
        if con.exists(public_key(&request.username)).await? {
            return Err(Abort(ClientError(UsernameTaken)));
        }

        if con.exists(meta(&root)).await? {
            return Err(Abort(internal!(
                "{} tried to create a new account with existing root {}",
                request.username,
                root.id
            )));
        }

        pipe_name
            .set_json(public_key(&request.username), request.public_key)?
            .set_json(owned_files(&request.username), [request.root_folder.id])?
            .set_json(meta(&root), &root)
    });
    return_if_error!(tx_result);

    Ok(NewAccountResponse {
        folder_metadata_version: root.metadata_version,
    })
}

pub async fn get_public_key(
    context: RequestContext<'_, GetPublicKeyRequest>,
) -> Result<GetPublicKeyResponse, ServerError<GetPublicKeyError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut con = server_state.index_db2_connection.get().await?;

    match con.maybe_get(public_key(&request.username)).await {
        Ok(Some(key)) => Ok(GetPublicKeyResponse { key }),
        Ok(None) => Err(ClientError(GetPublicKeyError::UserNotFound)),
        Err(err) => Err(internal!(
            "Error while getting public key for user: {}, err: {:?}",
            request.username,
            err
        )),
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
