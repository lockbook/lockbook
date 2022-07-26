use crate::schema::Account;
use crate::utils::username_is_valid;
use crate::ServerError::ClientError;
use crate::{document_service, RequestContext, ServerError, ServerState, Tx};
use hmdb::transaction::Transaction;
use libsecp256k1::PublicKey;
use lockbook_shared::api::NewAccountError::{FileIdTaken, PublicKeyTaken, UsernameTaken};
use lockbook_shared::api::{
    DeleteAccountError, DeleteAccountRequest, FileUsage, GetPublicKeyError, GetPublicKeyRequest,
    GetPublicKeyResponse, GetUsageError, GetUsageRequest, GetUsageResponse, NewAccountError,
    NewAccountRequest, NewAccountResponse,
};
use lockbook_shared::clock::get_time;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::server_file::IntoServerFile;

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
    let root = root.add_time(now);

    server_state.index_db.transaction(|tx| {
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

        Ok(NewAccountResponse { last_synced: root.metadata_version })
    })?
}

pub async fn get_public_key(
    context: RequestContext<'_, GetPublicKeyRequest>,
) -> Result<GetPublicKeyResponse, ServerError<GetPublicKeyError>> {
    let (request, server_state) = (&context.request, context.server_state);
    public_key_from_username(&request.username, server_state)
}

pub fn public_key_from_username(
    username: &str, server_state: &ServerState,
) -> Result<GetPublicKeyResponse, ServerError<GetPublicKeyError>> {
    server_state
        .index_db
        .usernames
        .get(&username.to_string())?
        .map(|owner| Ok(GetPublicKeyResponse { key: owner.0 }))
        .unwrap_or(Err(ClientError(GetPublicKeyError::UserNotFound)))
}

pub async fn get_usage(
    context: RequestContext<'_, GetUsageRequest>,
) -> Result<GetUsageResponse, ServerError<GetUsageError>> {
    context.server_state.index_db.transaction(|tx| {
        let cap = tx
            .accounts
            .get(&Owner(context.public_key))
            .ok_or(ClientError(GetUsageError::UserNotFound))?
            .billing_info
            .data_cap();
        let usages = get_usage_helper(tx, &context.public_key)?;
        Ok(GetUsageResponse { usages, cap })
    })?
}

#[derive(Debug)]
pub enum GetUsageHelperError {
    UserNotFound,
}

pub fn get_usage_helper(
    tx: &mut Tx<'_>, public_key: &PublicKey,
) -> Result<Vec<FileUsage>, GetUsageHelperError> {
    Ok(tx
        .owned_files
        .get(&Owner(*public_key))
        .ok_or(GetUsageHelperError::UserNotFound)?
        .into_iter()
        .filter_map(|&file_id| {
            tx.sizes
                .get(&file_id)
                .map(|&size_bytes| FileUsage { file_id, size_bytes })
        })
        .collect())
}

/// Delete's an account's files out of s3 and clears their file tree within redis
/// Does not free up the username or public key for re-use
pub async fn delete_account(
    context: RequestContext<'_, DeleteAccountRequest>,
) -> Result<(), ServerError<DeleteAccountError>> {
    let all_files: Result<EncryptedFiles, ServerError<DeleteAccountError>> =
        context.server_state.index_db.transaction(|tx| {
            let files: EncryptedFiles = tx
                .owned_files
                .get(&Owner(context.public_key))
                .ok_or(ClientError(DeleteAccountError::UserNotFound))?
                .iter()
                .filter_map(|id| tx.metas.get(id))
                .map(|f| (f.id, f))
                .collect();

            for file in files.values() {
                tx.metas.delete(file.id);
                if file.is_document() {
                    tx.sizes.delete(file.id);
                }
            }
            tx.owned_files.delete(Owner(context.public_key));

            if !context.server_state.config.is_prod() {
                let username = tx
                    .accounts
                    .delete(Owner(context.public_key))
                    .ok_or(ClientError(DeleteAccountError::UserNotFound))?
                    .username;
                tx.usernames.delete(username);
            }
            Ok(files)
        })?;

    let all_files = all_files?
        .filter_not_deleted()
        .map_err(|err| internal!("Could not get non-deleted files: {:?}", err))?;

    let non_deleted_document_ids = all_files.documents();

    for file in non_deleted_document_ids {
        let file = all_files
            .find(file)
            .map_err(|_| internal!("Could not find non-deleted file: {file}"))?;
        document_service::delete(context.server_state, file.id, file.content_version).await?;
    }

    Ok(())
}
