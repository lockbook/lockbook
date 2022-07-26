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
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::server_file::IntoServerFile;
use lockbook_shared::transaction::ServerTree;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use std::collections::HashSet;
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

    let root = request.root_folder.clone();
    let now = get_time().0 as u64;
    let root = root.add_time(now);

    server_state.index_db.transaction(|tx| {
        if tx.accounts.exists(&Owner(request.public_key)) {
            return Err(ClientError(PublicKeyTaken));
        }

        if tx.usernames.exists(&request.username) {
            return Err(ClientError(UsernameTaken));
        }

        if tx.metas.exists(root.id()) {
            return Err(ClientError(FileIdTaken));
        }

        let username = request.username;
        let account = Account { username: username.clone(), billing_info: Default::default() };

        let owner = Owner(request.public_key);

        let mut owned_files = HashSet::new();
        owned_files.insert(*root.id());

        tx.accounts.insert(owner.clone(), account);
        tx.usernames.insert(username, owner.clone());
        tx.owned_files.insert(owner, owned_files);
        tx.metas.insert(*root.id(), root.clone());

        Ok(NewAccountResponse { last_synced: root.version })
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

pub async fn delete_account(
    context: RequestContext<'_, DeleteAccountRequest>,
) -> Result<(), ServerError<DeleteAccountError>> {
    let all_files: Result<Vec<(Uuid, u64)>, ServerError<DeleteAccountError>> =
        context.server_state.index_db.transaction(|tx| {
            let mut tree = ServerTree {
                owner: Owner(context.public_key),
                owned: &mut tx.owned_files,
                metas: &mut tx.metas,
            }
            .to_lazy();
            let mut docs_to_delete = vec![];
            let metas_to_delete = tree.owned_ids();

            for id in tree.owned_ids() {
                if !tree.calculate_deleted(&id)? {
                    let meta = tree.find(&id)?;
                    if meta.is_document() {
                        docs_to_delete.push((*meta.id(), meta.version));
                        tx.sizes.delete(id);
                    }
                }
            }
            tx.owned_files.delete(Owner(context.public_key));
            for id in metas_to_delete {
                tx.metas.delete(id);
            }

            if !context.server_state.config.is_prod() {
                let username = tx
                    .accounts
                    .delete(Owner(context.public_key))
                    .ok_or(ClientError(DeleteAccountError::UserNotFound))?
                    .username;
                tx.usernames.delete(username);
            }
            Ok(docs_to_delete)
        })?;

    for (id, version) in all_files? {
        document_service::delete(context.server_state, id, version).await?;
    }

    Ok(())
}
