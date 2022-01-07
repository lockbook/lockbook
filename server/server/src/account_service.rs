use crate::utils::username_is_valid;
use crate::{keys, pipe, RequestContext, ServerError, FREE_TIER};
use deadpool_redis::redis::{AsyncCommands, Pipeline};
use lockbook_crypto::clock_service::get_time;
use log::error;
use redis_utils::converters::{JsonGet, JsonSet};

use redis_utils::{tx, TxError};
use uuid::Uuid;

use crate::keys::{data_cap, file, meta, owned_files, public_key, username};
use crate::ServerError::ClientError;
use lockbook_models::api::GetUsageError::UserNotFound;
use lockbook_models::api::NewAccountError::{FileIdTaken, PublicKeyTaken, UsernameTaken};
use lockbook_models::api::{
    FileUsage, GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse, GetUsageError,
    GetUsageRequest, GetUsageResponse, NewAccountError, NewAccountRequest, NewAccountResponse,
};

/// Create a new account given a username, public_key, and root folder.
/// Checks that username is valid, and that username, public_key and root_folder are new.
/// Inserts all of these values into their respective keys along with the default free account tier size
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
        username(&request.public_key),
        owned_files(&request.public_key),
        file(request.root_folder.id),
    ];

    let tx_result = tx!(&mut con, pipe_name, watched_keys, {
        if con.exists(public_key(&request.username)).await? {
            return Err(Abort(ClientError(UsernameTaken)));
        }

        if con.exists(username(&request.public_key)).await? {
            error!(
                "{} tried to use a public key that exists {}",
                &request.username,
                username(&request.public_key)
            );
            return Err(Abort(ClientError(PublicKeyTaken)));
        }

        if con.exists(meta(&root)).await? {
            error!(
                "{} tried to use a root that exists {}",
                &request.username,
                username(&request.public_key)
            );
            return Err(Abort(ClientError(FileIdTaken)));
        }

        pipe_name
            .json_set(public_key(&request.username), request.public_key)?
            .json_set(username(&request.public_key), &request.username)?
            .json_set(owned_files(&request.public_key), [request.root_folder.id])?
            .set(data_cap(&request.public_key), FREE_TIER)
            .json_set(meta(&root), &root)
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

    match con.maybe_json_get(public_key(&request.username)).await {
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
    let (_request, server_state) = (&context.request, context.server_state);
    let mut con = server_state.index_db2_connection.get().await?;

    let files: Vec<Uuid> = con
        .maybe_json_get(owned_files(&context.public_key))
        .await?
        .ok_or(ClientError(UserNotFound))?;

    let cap: u64 = con.get(data_cap(&context.public_key)).await?;

    let keys: Vec<String> = files.into_iter().map(keys::size).collect();

    let usages: Vec<FileUsage> = con.json_mget(keys).await?;

    Ok(GetUsageResponse { usages, cap })
}
