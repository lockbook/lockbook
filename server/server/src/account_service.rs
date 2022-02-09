use crate::utils::username_is_valid;
use crate::{feature_flags, keys, RequestContext, ServerError, ServerState, FREE_TIER};
use deadpool_redis::redis::AsyncCommands;
use lockbook_crypto::clock_service::get_time;
use log::{debug, error};
use redis_utils::converters::{JsonGet, JsonSet};

use redis_utils::tx;
use uuid::Uuid;

use crate::content::document_service;
use crate::keys::{data_cap, file, meta, owned_files, public_key, size, username};
use crate::ServerError::ClientError;
use lockbook_models::api::GetUsageError::UserNotFound;
use lockbook_models::api::NewAccountError::{FileIdTaken, PublicKeyTaken, UsernameTaken};
use lockbook_models::api::{
    DeleteAccountError, DeleteAccountRequest, FileUsage, GetPublicKeyError, GetPublicKeyRequest,
    GetPublicKeyResponse, GetUsageError, GetUsageRequest, GetUsageResponse, NewAccountError,
    NewAccountRequest, NewAccountResponse,
};
use lockbook_models::file_metadata::EncryptedFileMetadata;
use lockbook_models::file_metadata::FileType::Document;
use lockbook_models::tree::FileMetaExt;

/// Create a new account given a username, public_key, and root folder.
/// Checks that username is valid, and that username, public_key and root_folder are new.
/// Inserts all of these values into their respective keys along with the default free account tier size
pub async fn new_account(
    context: RequestContext<'_, NewAccountRequest>,
) -> Result<NewAccountResponse, ServerError<NewAccountError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let request =
        NewAccountRequest { username: request.username.to_lowercase(), ..request.clone() };

    if !username_is_valid(&request.username) {
        return Err(ClientError(NewAccountError::InvalidUsername));
    }

    let mut con = server_state.index_db_pool.get().await?;

    if !feature_flags::is_new_accounts_enabled(&mut con).await? {
        return Err(ClientError(NewAccountError::Disabled));
    }

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

    Ok(NewAccountResponse { folder_metadata_version: root.metadata_version })
}

pub async fn get_public_key(
    context: RequestContext<'_, GetPublicKeyRequest>,
) -> Result<GetPublicKeyResponse, ServerError<GetPublicKeyError>> {
    let (request, server_state) = (&context.request, context.server_state);
    Ok(public_key_from_username(&request.username, server_state).await?)
}

pub async fn public_key_from_username(
    username: &str, server_state: &ServerState,
) -> Result<GetPublicKeyResponse, ServerError<GetPublicKeyError>> {
    let mut con = server_state.index_db_pool.get().await?;

    match con.maybe_json_get(public_key(username)).await {
        Ok(Some(key)) => Ok(GetPublicKeyResponse { key }),
        Ok(None) => Err(ClientError(GetPublicKeyError::UserNotFound)),
        Err(err) => {
            Err(internal!("Error while getting public key for user: {}, err: {:?}", username, err))
        }
    }
}

pub async fn get_usage(
    context: RequestContext<'_, GetUsageRequest>,
) -> Result<GetUsageResponse, ServerError<GetUsageError>> {
    let (_request, server_state) = (&context.request, context.server_state);
    let mut con = server_state.index_db_pool.get().await?;

    let files: Vec<Uuid> = con
        .maybe_json_get(owned_files(&context.public_key))
        .await?
        .ok_or(ClientError(UserNotFound))?;

    let cap: u64 = con.get(data_cap(&context.public_key)).await?;

    let keys: Vec<String> = files.into_iter().map(keys::size).collect();

    let usages: Vec<FileUsage> = con.json_mget(keys).await?;

    Ok(GetUsageResponse { usages, cap })
}

/// Delete's an account's files out of s3 and clears their file tree within redis
/// Does not free up the username or public key for re-use
pub async fn delete_account(
    context: RequestContext<'_, DeleteAccountRequest>,
) -> Result<(), ServerError<DeleteAccountError>> {
    let mut con = context.server_state.index_db_pool.get().await?;
    let mut all_files: Vec<EncryptedFileMetadata> = vec![];

    let tx = tx!(&mut con, pipe, &[owned_files(&context.public_key)], {
        let files: Vec<Uuid> = con
            .maybe_json_get(owned_files(&context.public_key))
            .await?
            .ok_or(Abort(ClientError(DeleteAccountError::UserNotFound)))?;
        let keys: Vec<String> = files.into_iter().map(keys::file).collect();
        let files: Vec<EncryptedFileMetadata> = con.watch_json_mget(keys).await?;

        all_files = files.clone();

        for the_file in &files {
            pipe.del(file(the_file.id));
            if the_file.file_type == Document {
                pipe.del(size(the_file.id));
            }
        }
        pipe.del(owned_files(&context.public_key));
        Ok(&mut pipe)
    });
    return_if_error!(tx);

    let non_deleted_document = all_files
        .filter_not_deleted()
        .map_err(|err| internal!("Could not get non-deleted files: {:?}", err))?
        .filter_documents();

    for file in non_deleted_document {
        document_service::delete(context.server_state, file.id, file.content_version).await?;
    }

    Ok(())
}

/// Delete's an account's files out of s3 and clears their file tree within redis
/// DOES free up the username or public key for re-use, not exposed for non-admin use
pub async fn purge_account(
    context: RequestContext<'_, DeleteAccountRequest>,
) -> Result<(), ServerError<DeleteAccountError>> {
    let mut con = context.server_state.index_db_pool.get().await?;
    // Delete the files
    delete_account(context.clone()).await?;

    // Delete everything else
    let username: String = con.get(keys::username(&context.public_key)).await?;
    debug!("purging username: {}", username);
    con.del(keys::username(&context.public_key)).await?;
    con.del(public_key(&username)).await?;
    con.del(data_cap(&context.public_key)).await?;
    Ok(())
}
