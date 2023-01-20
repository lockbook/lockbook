use crate::billing::billing_model::BillingPlatform;
use crate::schema::Account;
use crate::utils::username_is_valid;
use crate::ServerError::ClientError;
use crate::{document_service, RequestContext, Server, ServerError, ServerState, Tx};
use hmdb::transaction::Transaction;
use libsecp256k1::PublicKey;
use lockbook_shared::account::Username;
use lockbook_shared::api::NewAccountError::{FileIdTaken, PublicKeyTaken, UsernameTaken};
use lockbook_shared::api::{
    AccountFilter, AccountIdentifier, AccountInfo, AdminDisappearAccountError,
    AdminDisappearAccountRequest, AdminGetAccountInfoError, AdminGetAccountInfoRequest,
    AdminGetAccountInfoResponse, AdminListUsersError, AdminListUsersRequest,
    AdminListUsersResponse, DeleteAccountError, DeleteAccountRequest, FileUsage, GetPublicKeyError,
    GetPublicKeyRequest, GetPublicKeyResponse, GetUsageError, GetUsageRequest, GetUsageResponse,
    GetUsernameError, GetUsernameRequest, GetUsernameResponse, NewAccountError, NewAccountRequest,
    NewAccountResponse, PaymentPlatform,
};
use lockbook_shared::clock::get_time;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{DocumentHmac, Owner};
use lockbook_shared::server_file::IntoServerFile;
use lockbook_shared::server_tree::ServerTree;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::usage::bytes_to_human;
use std::collections::HashSet;
use std::fmt::Debug;
use tracing::warn;
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

        tx.accounts.insert(owner, account);
        tx.usernames.insert(username, owner);
        tx.owned_files.insert(owner, owned_files);
        tx.shared_files.insert(owner, HashSet::new());
        tx.file_children.insert(*root.id(), HashSet::new());
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

pub async fn get_username(
    context: RequestContext<'_, GetUsernameRequest>,
) -> Result<GetUsernameResponse, ServerError<GetUsernameError>> {
    let (request, server_state) = (&context.request, context.server_state);
    username_from_public_key(request.key, server_state)
}

pub fn username_from_public_key(
    key: PublicKey, server_state: &ServerState,
) -> Result<GetUsernameResponse, ServerError<GetUsernameError>> {
    server_state
        .index_db
        .accounts
        .get(&Owner(key))?
        .map(|account| Ok(GetUsernameResponse { username: account.username }))
        .unwrap_or(Err(ClientError(GetUsernameError::UserNotFound)))
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
        .iter()
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
    delete_account_helper(context.server_state, &context.public_key, false).await?;

    Ok(())
}

pub async fn admin_disappear_account(
    context: RequestContext<'_, AdminDisappearAccountRequest>,
) -> Result<(), ServerError<AdminDisappearAccountError>> {
    let db = &context.server_state.index_db;

    if !is_admin::<AdminDisappearAccountError>(
        db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(AdminDisappearAccountError::NotPermissioned));
    }

    let admin_username = db
        .accounts
        .get(&Owner(context.public_key))?
        .map(|account| account.username)
        .unwrap_or_else(|| "~unknown~".to_string());

    warn!("admin {} is disappearing account {}", admin_username, context.request.username);

    let owner = db
        .usernames
        .get(&context.request.username)?
        .ok_or(ClientError(AdminDisappearAccountError::UserNotFound))?;

    delete_account_helper(context.server_state, &owner.0, true).await?;

    Ok(())
}

pub async fn admin_list_users(
    context: RequestContext<'_, AdminListUsersRequest>,
) -> Result<AdminListUsersResponse, ServerError<AdminListUsersError>> {
    let (db, request) = (&context.server_state.index_db, &context.request);

    if !is_admin::<AdminListUsersError>(
        db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(AdminListUsersError::NotPermissioned));
    }

    let mut users: Vec<String> = vec![];

    for account in db.accounts.get_all()?.values() {
        match &request.filter {
            Some(filter) => match filter {
                AccountFilter::Premium => {
                    if account.billing_info.billing_platform.is_some() {
                        users.push(account.username.clone());
                    }
                }
                AccountFilter::StripePremium => {
                    if let Some(BillingPlatform::Stripe(_)) = account.billing_info.billing_platform
                    {
                        users.push(account.username.clone());
                    }
                }
                AccountFilter::GooglePlayPremium => {
                    if let Some(BillingPlatform::GooglePlay(_)) =
                        account.billing_info.billing_platform
                    {
                        users.push(account.username.clone());
                    }
                }
            },
            None => users.push(account.username.clone()),
        }
    }

    Ok(AdminListUsersResponse { users })
}

pub async fn admin_get_account_info(
    context: RequestContext<'_, AdminGetAccountInfoRequest>,
) -> Result<AdminGetAccountInfoResponse, ServerError<AdminGetAccountInfoError>> {
    let (db, request) = (&context.server_state.index_db, &context.request);

    if !is_admin::<AdminGetAccountInfoError>(
        db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(AdminGetAccountInfoError::NotPermissioned));
    }

    let owner = match &request.identifier {
        AccountIdentifier::PublicKey(public_key) => Owner(*public_key),
        AccountIdentifier::Username(user) => db
            .usernames
            .get(user)?
            .ok_or(ClientError(AdminGetAccountInfoError::UserNotFound))?,
    };

    let account = db
        .accounts
        .get(&owner)?
        .ok_or(ClientError(AdminGetAccountInfoError::UserNotFound))?;

    let mut maybe_root = None;
    if let Some(owned_ids) = db.owned_files.get(&owner)? {
        for id in owned_ids {
            if let Some(meta) = db.metas.get(&id)? {
                if meta.is_root() {
                    maybe_root = Some(*meta.id());
                }
            } else {
                return Err(internal!(
                    "Nonexistent file indexed as owned, id: {}, owner: {:?}",
                    id,
                    owner
                ));
            }
        }
    } else {
        return Err(internal!("Owned files not indexed for user, owner: {:?}", owner));
    }
    let root = if let Some(root) = maybe_root {
        root
    } else {
        return Err(internal!("User root not found, owner: {:?}", owner));
    };

    let payment_platform = account
        .billing_info
        .billing_platform
        .map(|billing_platform| match billing_platform {
            BillingPlatform::Stripe(user_info) => {
                PaymentPlatform::Stripe { card_last_4_digits: user_info.last_4 }
            }
            BillingPlatform::GooglePlay(user_info) => {
                PaymentPlatform::GooglePlay { account_state: user_info.account_state }
            }
            BillingPlatform::AppStore(user_info) => {
                PaymentPlatform::AppStore { account_state: user_info.account_state }
            }
        });

    let usage: u64 = db
        .transaction(|tx| get_usage_helper(tx, &owner.0))?
        .map_err(|err| internal!("Cannot find user's usage, owner: {:?}, err: {:?}", owner, err))?
        .iter()
        .map(|a| a.size_bytes)
        .sum();

    let usage_str = bytes_to_human(usage);

    Ok(AdminGetAccountInfoResponse {
        account: AccountInfo {
            username: account.username,
            root,
            payment_platform,
            usage: usage_str,
        },
    })
}

#[derive(Debug)]
pub enum DeleteAccountHelperError {
    UserNotFound,
}

pub async fn delete_account_helper(
    server_state: &ServerState, public_key: &PublicKey, free_username: bool,
) -> Result<(), ServerError<DeleteAccountHelperError>> {
    let all_files: Result<Vec<(Uuid, DocumentHmac)>, ServerError<DeleteAccountHelperError>> =
        server_state.index_db.transaction(|tx| {
            let mut tree = ServerTree::new(
                Owner(*public_key),
                &mut tx.owned_files,
                &mut tx.shared_files,
                &mut tx.file_children,
                &mut tx.metas,
            )?
            .to_lazy();
            let mut docs_to_delete = vec![];
            let metas_to_delete = tree.owned_ids();

            for id in tree.owned_ids() {
                if !tree.calculate_deleted(&id)? {
                    let meta = tree.find(&id)?;
                    if meta.is_document() {
                        if let Some(hmac) = meta.document_hmac() {
                            docs_to_delete.push((*meta.id(), *hmac));
                            tx.sizes.delete(id);
                        }
                    }
                }
            }
            tx.owned_files.delete(Owner(*public_key));
            tx.shared_files.delete(Owner(*public_key));
            tx.last_seen.delete(Owner(*public_key));

            for id in metas_to_delete {
                if let Some(meta) = tx.metas.get(&id) {
                    if &(meta.owner().0) == public_key {
                        for user_access_key in meta.user_access_keys() {
                            let sharee = Owner(user_access_key.encrypted_for);
                            if let Some(shared_files) = tx.shared_files.get(&sharee) {
                                let new_shared_files = shared_files
                                    .iter()
                                    .copied()
                                    .filter(|id| id != meta.id())
                                    .collect();
                                tx.shared_files.insert(sharee, new_shared_files);
                            }
                        }
                        tx.metas.delete(id);
                        tx.file_children.delete(id);
                    }
                }
            }

            if free_username {
                let username = tx
                    .accounts
                    .delete(Owner(*public_key))
                    .ok_or(ClientError(DeleteAccountHelperError::UserNotFound))?
                    .username;
                tx.usernames.delete(username);
            }
            Ok(docs_to_delete)
        })?;

    for (id, version) in all_files? {
        document_service::delete(server_state, &id, &version).await?;
    }

    Ok(())
}

pub fn is_admin<E: Debug>(
    db: &Server, public_key: &PublicKey, admins: &HashSet<Username>,
) -> Result<bool, ServerError<E>> {
    let is_admin = match db.accounts.get(&Owner(*public_key))? {
        None => false,
        Some(account) => admins
            .iter()
            .any(|admin_username| *admin_username == account.username),
    };

    Ok(is_admin)
}
