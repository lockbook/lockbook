use crate::billing::billing_model::BillingPlatform;
use crate::schema::{Account, ServerDb};
use crate::utils::username_is_valid;
use crate::ServerError::ClientError;
use crate::{document_service, RequestContext, ServerError, ServerState};
use db_rs::Db;
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
    NewAccountResponse, PaymentPlatform, METADATA_FEE,
};
use lockbook_shared::clock::get_time;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::lazy::LazyTree;
use lockbook_shared::server_file::IntoServerFile;
use lockbook_shared::server_tree::ServerTree;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::usage::bytes_to_human;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::ops::DerefMut;
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

    let mut db = server_state.index_db.lock()?;
    let handle = db.begin_transaction()?;

    if db.accounts.data().contains_key(&Owner(request.public_key)) {
        return Err(ClientError(PublicKeyTaken));
    }

    if db.usernames.data().contains_key(&request.username) {
        return Err(ClientError(UsernameTaken));
    }

    if db.metas.data().contains_key(root.id()) {
        return Err(ClientError(FileIdTaken));
    }

    let username = request.username;
    let account = Account { username: username.clone(), billing_info: Default::default() };

    let owner = Owner(request.public_key);

    let mut owned_files = HashSet::new();
    owned_files.insert(*root.id());

    db.accounts.insert(owner, account)?;
    db.usernames.insert(username, owner)?;
    db.owned_files.insert(owner, *root.id())?;
    db.shared_files.create_key(owner)?;
    db.file_children.create_key(*root.id())?;
    db.metas.insert(*root.id(), root.clone())?;

    handle.drop_safely()?;

    Ok(NewAccountResponse { last_synced: root.version })
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
        .lock()?
        .usernames
        .data()
        .get(username)
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
        .lock()?
        .accounts
        .data()
        .get(&Owner(key))
        .map(|account| Ok(GetUsernameResponse { username: account.username.clone() }))
        .unwrap_or(Err(ClientError(GetUsernameError::UserNotFound)))
}

pub async fn get_usage(
    context: RequestContext<'_, GetUsageRequest>,
) -> Result<GetUsageResponse, ServerError<GetUsageError>> {
    let mut lock = context.server_state.index_db.lock()?;
    let db = lock.deref_mut();

    let cap = get_cap(db, &context.public_key)?;

    let mut tree = ServerTree::new(
        Owner(context.public_key),
        &mut db.owned_files,
        &mut db.shared_files,
        &mut db.file_children,
        &mut db.metas,
    )?
    .to_lazy();

    let usages = get_usage_helper(&mut tree, db.sizes.data())?;
    Ok(GetUsageResponse { usages, cap })
}

#[derive(Debug)]
pub enum GetUsageHelperError {
    UserNotFound,
}

pub fn get_usage_helper<T>(
    tree: &mut LazyTree<T>, sizes: &HashMap<Uuid, u64>,
) -> Result<Vec<FileUsage>, GetUsageHelperError>
where
    T: TreeLike,
{
    let ids = tree.owned_ids();
    let root = ids
        .iter()
        .find(|file_id| match tree.find(file_id) {
            Ok(f) => f.is_root(),
            Err(_) => false,
        })
        .ok_or(GetUsageHelperError::UserNotFound)?;

    let result = ids
        .iter()
        .filter_map(|&file_id| {
            if let Ok(file) = tree.find(&file_id) {
                if file.owner() != tree.find(root).unwrap().owner() {
                    return None;
                }
            } else {
                return None;
            }

            let file_size = match tree.calculate_deleted(&file_id).unwrap_or(true) {
                true => 0,
                false => *sizes.get(&file_id).unwrap_or(&0),
            };

            Some(FileUsage { file_id, size_bytes: file_size + METADATA_FEE })
        })
        .collect();
    Ok(result)
}

pub fn get_cap(db: &ServerDb, public_key: &PublicKey) -> Result<u64, GetUsageHelperError> {
    Ok(db
        .accounts
        .data()
        .get(&Owner(*public_key))
        .ok_or(GetUsageHelperError::UserNotFound)?
        .billing_info
        .data_cap())
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
    let owner = {
        let db = context.server_state.index_db.lock()?;

        if !is_admin::<AdminDisappearAccountError>(
            &db,
            &context.public_key,
            &context.server_state.config.admin.admins,
        )? {
            return Err(ClientError(AdminDisappearAccountError::NotPermissioned));
        }

        let admin_username = db
            .accounts
            .data()
            .get(&Owner(context.public_key))
            .cloned()
            .map(|account| account.username)
            .unwrap_or_else(|| "~unknown~".to_string());

        warn!("admin {} is disappearing account {}", admin_username, context.request.username);

        *db.usernames
            .data()
            .get(&context.request.username)
            .ok_or(ClientError(AdminDisappearAccountError::UserNotFound))?
    };

    delete_account_helper(context.server_state, &owner.0, true).await?;

    Ok(())
}

pub async fn admin_list_users(
    context: RequestContext<'_, AdminListUsersRequest>,
) -> Result<AdminListUsersResponse, ServerError<AdminListUsersError>> {
    let (db, request) = (context.server_state.index_db.lock()?, &context.request);

    if !is_admin::<AdminListUsersError>(
        &db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(AdminListUsersError::NotPermissioned));
    }

    let mut users: Vec<String> = vec![];

    for account in db.accounts.data().values() {
        match &request.filter {
            Some(filter) => match filter {
                AccountFilter::Premium => {
                    if account.billing_info.is_premium() {
                        users.push(account.username.clone());
                    }
                }
                AccountFilter::AppStorePremium => match account.billing_info.billing_platform {
                    Some(BillingPlatform::AppStore(_)) if account.billing_info.is_premium() => {
                        users.push(account.username.clone());
                    }
                    _ => {}
                },
                AccountFilter::StripePremium => match account.billing_info.billing_platform {
                    Some(BillingPlatform::Stripe(_)) if account.billing_info.is_premium() => {
                        users.push(account.username.clone());
                    }
                    _ => {}
                },
                AccountFilter::GooglePlayPremium => match account.billing_info.billing_platform {
                    Some(BillingPlatform::GooglePlay(_)) if account.billing_info.is_premium() => {
                        users.push(account.username.clone());
                    }
                    _ => {}
                },
            },
            None => users.push(account.username.clone()),
        }
    }

    Ok(AdminListUsersResponse { users })
}

pub async fn admin_get_account_info(
    context: RequestContext<'_, AdminGetAccountInfoRequest>,
) -> Result<AdminGetAccountInfoResponse, ServerError<AdminGetAccountInfoError>> {
    let (mut lock, request) = (context.server_state.index_db.lock()?, &context.request);
    let db = lock.deref_mut();

    if !is_admin::<AdminGetAccountInfoError>(
        db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(AdminGetAccountInfoError::NotPermissioned));
    }

    let owner = match &request.identifier {
        AccountIdentifier::PublicKey(public_key) => Owner(*public_key),
        AccountIdentifier::Username(user) => *db
            .usernames
            .data()
            .get(user)
            .ok_or(ClientError(AdminGetAccountInfoError::UserNotFound))?,
    };

    let account = db
        .accounts
        .data()
        .get(&owner)
        .ok_or(ClientError(AdminGetAccountInfoError::UserNotFound))?
        .clone();

    let mut maybe_root = None;
    if let Some(owned_ids) = db.owned_files.data().get(&owner) {
        for id in owned_ids {
            if let Some(meta) = db.metas.data().get(id) {
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

    let mut tree = ServerTree::new(
        owner,
        &mut db.owned_files,
        &mut db.shared_files,
        &mut db.file_children,
        &mut db.metas,
    )?
    .to_lazy();

    let usage: u64 = get_usage_helper(&mut tree, db.sizes.data())
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
    let mut docs_to_delete = Vec::new();

    {
        let mut lock = server_state.index_db.lock()?;
        let db = lock.deref_mut();
        let tx = db.begin_transaction()?;

        let mut tree = ServerTree::new(
            Owner(*public_key),
            &mut db.owned_files,
            &mut db.shared_files,
            &mut db.file_children,
            &mut db.metas,
        )?
        .to_lazy();
        let metas_to_delete = tree.owned_ids();

        for id in metas_to_delete.clone() {
            if !tree.calculate_deleted(&id)? {
                let meta = tree.find(&id)?;
                if meta.is_document() && &(meta.owner().0) == public_key {
                    if let Some(hmac) = meta.document_hmac() {
                        docs_to_delete.push((*meta.id(), *hmac));
                        db.sizes.remove(&id)?;
                    }
                }
            }
        }
        db.owned_files.clear_key(&Owner(*public_key))?;
        db.shared_files.clear_key(&Owner(*public_key))?;
        db.last_seen.remove(&Owner(*public_key))?;

        for id in metas_to_delete {
            if let Some(meta) = db.metas.data().get(&id) {
                if &(meta.owner().0) == public_key {
                    for user_access_key in meta.user_access_keys() {
                        let sharee = Owner(user_access_key.encrypted_for);
                        db.shared_files.remove(&sharee, meta.id())?;
                    }
                    db.metas.remove(&id)?;
                    db.file_children.clear_key(&id)?;
                }
            }
        }

        if free_username {
            let username = db
                .accounts
                .remove(&Owner(*public_key))?
                .ok_or(ClientError(DeleteAccountHelperError::UserNotFound))?
                .username;
            db.usernames.remove(&username)?;
        }

        tx.drop_safely()?;
        drop(lock);
    }

    for (id, version) in docs_to_delete {
        document_service::delete(server_state, &id, &version).await?;
    }
    Ok(())
}

pub fn is_admin<E: Debug>(
    db: &ServerDb, public_key: &PublicKey, admins: &HashSet<Username>,
) -> Result<bool, ServerError<E>> {
    let is_admin = match db.accounts.data().get(&Owner(*public_key)) {
        None => false,
        Some(account) => admins.contains(&account.username),
    };

    Ok(is_admin)
}
