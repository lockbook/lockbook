use crate::billing::billing_model::SubscriptionProfile;
use crate::content::document_service;
use crate::keys::{file, owned_files, public_key, size, subscription_profile};
use crate::schema::Account;
use crate::utils::username_is_valid;
use crate::ServerError::ClientError;
use crate::{keys, RequestContext, ServerError, ServerState};
use deadpool_redis::redis::AsyncCommands;
use deadpool_redis::Connection;
use hmdb::transaction::Transaction;
use libsecp256k1::PublicKey;
use lockbook_crypto::clock_service::get_time;
use lockbook_models::api::NewAccountError::{FileIdTaken, PublicKeyTaken, UsernameTaken};
use lockbook_models::api::{
    DeleteAccountError, DeleteAccountRequest, FileUsage, GetPublicKeyError, GetPublicKeyRequest,
    GetPublicKeyResponse, GetUsageError, GetUsageRequest, GetUsageResponse, NewAccountError,
    NewAccountRequest, NewAccountResponse,
};
use lockbook_models::file_metadata::{EncryptedFileMetadata, EncryptedFiles, Owner};
use lockbook_models::tree::{FileMetaMapExt, FileMetaVecExt, FileMetadata};
use log::debug;
use redis_utils::converters::JsonGet;
use redis_utils::tx;
use std::collections::HashMap;
use uuid::Uuid;

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

    if !context.server_state.config.features.new_accounts {
        return Err(ClientError(NewAccountError::Disabled));
    }

    let mut root = request.root_folder.clone();
    let now = get_time().0 as u64;
    root.metadata_version = now;

    server_state
        .index_db
        .transaction(|tx| {
            if tx.accounts.exists(&Owner(request.public_key)) {
                return Err(ClientError(PublicKeyTaken));
            }

            if tx.usernames.exists(&request.username) {
                return Err(ClientError(UsernameTaken));
            }

            if tx.metas.exists(&root.id) {
                return Err(ClientError(FileIdTaken));
            }

            let username = request.username;
            let account = Account { username: username.clone(), billing_info: Default::default() };

            let owner = Owner(request.public_key);

            tx.accounts.insert(owner.clone(), account);
            tx.usernames.insert(username, owner.clone());
            tx.owned_files.insert(owner, vec![root.id]);
            tx.metas.insert(root.id, root.clone());

            Ok(NewAccountResponse { folder_metadata_version: root.metadata_version })
        })
        .unwrap()
}

pub async fn get_public_key(
    context: RequestContext<'_, GetPublicKeyRequest>,
) -> Result<GetPublicKeyResponse, ServerError<GetPublicKeyError>> {
    let (request, server_state) = (&context.request, context.server_state);
    public_key_from_username(&request.username, server_state).await
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

    let sub_profile: SubscriptionProfile = con
        .json_get(keys::subscription_profile(&context.public_key))
        .await?;

    let usages = get_usage_helper(&mut con, &context.public_key).await?;

    Ok(GetUsageResponse { usages, cap: sub_profile.data_cap() })
}

#[derive(Debug)]
pub enum GetUsageHelperError {
    UserNotFound,
    Internal(redis_utils::converters::JsonGetError),
}

pub async fn get_usage_helper(
    con: &mut deadpool_redis::Connection, public_key: &PublicKey,
) -> Result<Vec<FileUsage>, GetUsageHelperError> {
    let files: Vec<Uuid> = con
        .maybe_json_get(owned_files(public_key))
        .await
        .map_err(GetUsageHelperError::Internal)?
        .ok_or(GetUsageHelperError::UserNotFound)?;

    let keys: Vec<String> = files.into_iter().map(keys::size).collect();

    con.json_mget(keys)
        .await
        .map_err(GetUsageHelperError::Internal)
}

/// Delete's an account's files out of s3 and clears their file tree within redis
/// Does not free up the username or public key for re-use
pub async fn delete_account(
    context: RequestContext<'_, DeleteAccountRequest>,
) -> Result<(), ServerError<DeleteAccountError>> {
    let mut con = context.server_state.index_db_pool.get().await?;
    let mut all_files: EncryptedFiles = HashMap::new();

    let tx = tx!(&mut con, pipe, &[owned_files(&context.public_key)], {
        let files: Vec<Uuid> = con
            .maybe_json_get(owned_files(&context.public_key))
            .await?
            .ok_or(Abort(ClientError(DeleteAccountError::UserNotFound)))?;
        let keys: Vec<String> = files.into_iter().map(keys::file).collect();
        let files: Vec<EncryptedFileMetadata> = con.watch_json_mget(keys).await?;

        all_files = files.to_map();

        for the_file in &files {
            pipe.del(file(the_file.id));
            if the_file.is_document() {
                pipe.del(size(the_file.id));
            }
        }
        pipe.del(owned_files(&context.public_key));
        Ok(&mut pipe)
    });
    return_if_error!(tx);

    let all_files = all_files
        .filter_not_deleted()
        .map_err(|err| internal!("Could not get non-deleted files: {:?}", err))?;

    let non_deleted_document_ids = all_files.documents();

    for file in non_deleted_document_ids {
        let file = all_files
            .find(file)
            .map_err(|_| internal!("Could not find non-deleted file: {file}"))?;
        document_service::delete(context.server_state, file.id, file.content_version).await?;
    }

    if !context.server_state.config.is_prod() {
        free_username(&mut con, &context.public_key).await?;
    }

    Ok(())
}

/// Delete's an account's files out of s3 and clears their file tree within redis
/// DOES free up the username or public key for re-use, not exposed for non-admin use
pub async fn free_username(
    con: &mut Connection, pk: &PublicKey,
) -> Result<(), ServerError<DeleteAccountError>> {
    // Delete everything else
    let username: String = con.json_get(keys::username(pk)).await?;
    debug!("purging username: {}", username);
    con.del(keys::username(pk)).await?;
    con.del(public_key(&username)).await?;
    con.del(subscription_profile(pk)).await?;
    Ok(())
}
