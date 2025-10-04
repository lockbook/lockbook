use crate::ServerError::ClientError;
use crate::billing::app_store_client::AppStoreClient;
use crate::billing::billing_model::BillingPlatform;
use crate::billing::google_play_client::GooglePlayClient;
use crate::billing::stripe_client::StripeClient;
use crate::document_service::DocumentService;
use crate::schema::{Account, ServerDb};
use crate::utils::username_is_valid;
use crate::{RequestContext, ServerError, ServerState};
use db_rs::Db;
use lb_rs::model::account::Username;
use lb_rs::model::api::NewAccountError::{FileIdTaken, PublicKeyTaken, UsernameTaken};
use lb_rs::model::api::{
    AccountFilter, AccountIdentifier, AccountInfo, AdminDisappearAccountError,
    AdminDisappearAccountRequest, AdminGetAccountInfoError, AdminGetAccountInfoRequest,
    AdminGetAccountInfoResponse, AdminListUsersError, AdminListUsersRequest,
    AdminListUsersResponse, DeleteAccountError, DeleteAccountRequest, FileUsage, GetPublicKeyError,
    GetPublicKeyRequest, GetPublicKeyResponse, GetUsageError, GetUsageRequest, GetUsageResponse,
    GetUsernameError, GetUsernameRequest, GetUsernameResponse, METADATA_FEE, NewAccountError,
    NewAccountRequest, NewAccountRequestV2, NewAccountResponse, PaymentPlatform,
};
use lb_rs::model::clock::get_time;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::Owner;
use lb_rs::model::lazy::LazyTree;
use lb_rs::model::server_meta::{IntoServerMeta, ServerMeta};
use lb_rs::model::server_tree::ServerTree;
use lb_rs::model::signed_meta::SignedMeta;
use lb_rs::model::tree_like::TreeLike;
use lb_rs::model::usage::bytes_to_human;
use libsecp256k1::PublicKey;
use std::collections::HashSet;
use std::fmt::Debug;
use std::ops::DerefMut;
use tracing::warn;

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    /// Create a new account given a username, public_key, and root folder.
    /// Checks that username is valid, and that username, public_key and root_folder are new.
    /// Inserts all of these values into their respective keys along with the default free account tier size
    pub async fn new_account(
        &self, context: RequestContext<NewAccountRequest>,
    ) -> Result<NewAccountResponse, ServerError<NewAccountError>> {
        let request = context.request;
        let request = NewAccountRequestV2 {
            username: request.username.to_lowercase(),
            public_key: request.public_key,
            root_folder: SignedMeta::from(request.root_folder),
        };

        self.new_account_v2(RequestContext {
            request,
            public_key: context.public_key,
            ip: context.ip,
        })
        .await
    }

    /// Create a new account given a username, public_key, and root folder.
    /// Checks that username is valid, and that username, public_key and root_folder are new.
    /// Inserts all of these values into their respective keys along with the default free account tier size
    pub async fn new_account_v2(
        &self, mut context: RequestContext<NewAccountRequestV2>,
    ) -> Result<NewAccountResponse, ServerError<NewAccountError>> {
        context.request.username = context.request.username.to_lowercase();
        let request = &context.request;

        tracing::info!("new-account attempt username: {}", request.username);

        if !username_is_valid(&request.username) {
            return Err(ClientError(NewAccountError::InvalidUsername));
        }

        if !&self.config.features.new_accounts {
            return Err(ClientError(NewAccountError::Disabled));
        }

        let root = request.root_folder.clone();
        let now = get_time().0 as u64;
        let root = root.add_time(now);

        let mut db = self.index_db.lock().await;
        let handle = db.begin_transaction()?;

        if db.accounts.get().contains_key(&Owner(request.public_key)) {
            return Err(ClientError(PublicKeyTaken));
        }

        if db.usernames.get().contains_key(&request.username) {
            return Err(ClientError(UsernameTaken));
        }

        if db.metas.get().contains_key(root.id()) {
            return Err(ClientError(FileIdTaken));
        }

        let username = &request.username;
        let account = Account { username: username.clone(), billing_info: Default::default() };

        let owner = Owner(request.public_key);

        let mut owned_files = HashSet::new();
        owned_files.insert(*root.id());

        db.accounts.insert(owner, account)?;
        db.usernames.insert(username.clone(), owner)?;
        db.owned_files.insert(owner, *root.id())?;
        db.shared_files.create_key(owner)?;
        db.file_children.create_key(*root.id())?;
        db.metas.insert(*root.id(), root.clone())?;

        handle.drop_safely()?;

        Ok(NewAccountResponse { last_synced: root.version })
    }

    pub async fn get_public_key(
        &self, context: RequestContext<GetPublicKeyRequest>,
    ) -> Result<GetPublicKeyResponse, ServerError<GetPublicKeyError>> {
        let request = &context.request;
        self.public_key_from_username(&request.username).await
    }

    pub async fn public_key_from_username(
        &self, username: &str,
    ) -> Result<GetPublicKeyResponse, ServerError<GetPublicKeyError>> {
        self.index_db
            .lock()
            .await
            .usernames
            .get()
            .get(username)
            .map(|owner| Ok(GetPublicKeyResponse { key: owner.0 }))
            .unwrap_or(Err(ClientError(GetPublicKeyError::UserNotFound)))
    }

    pub async fn get_username(
        &self, context: RequestContext<GetUsernameRequest>,
    ) -> Result<GetUsernameResponse, ServerError<GetUsernameError>> {
        self.username_from_public_key(context.request.key).await
    }

    pub async fn username_from_public_key(
        &self, key: PublicKey,
    ) -> Result<GetUsernameResponse, ServerError<GetUsernameError>> {
        self.index_db
            .lock()
            .await
            .accounts
            .get()
            .get(&Owner(key))
            .map(|account| Ok(GetUsernameResponse { username: account.username.clone() }))
            .unwrap_or(Err(ClientError(GetUsernameError::UserNotFound)))
    }

    pub async fn get_usage(
        &self, context: RequestContext<GetUsageRequest>,
    ) -> Result<GetUsageResponse, ServerError<GetUsageError>> {
        let mut lock = self.index_db.lock().await;
        let db = lock.deref_mut();

        let cap = Self::get_cap(db, &context.public_key)?;

        let mut tree = ServerTree::new(
            Owner(context.public_key),
            &mut db.owned_files,
            &mut db.shared_files,
            &mut db.file_children,
            &mut db.metas,
        )?
        .to_lazy();
        let usages = Self::get_usage_helper(&mut tree)?;
        Ok(GetUsageResponse { usages, cap })
    }

    pub fn get_usage_helper<T>(
        tree: &mut LazyTree<T>,
    ) -> Result<Vec<FileUsage>, ServerError<GetUsageHelperError>>
    where
        T: TreeLike<F = ServerMeta>,
    {
        let ids = tree.ids();
        let root_id = ids
            .iter()
            .find(|file_id| match tree.find(file_id) {
                Ok(f) => f.is_root(),
                Err(_) => false,
            })
            .ok_or(ClientError(GetUsageHelperError::UserDeleted))?;

        let root_owner = tree
            .maybe_find(root_id)
            .ok_or(ClientError(GetUsageHelperError::UserDeleted))?
            .owner();

        let result = ids
            .iter()
            .filter_map(|&file_id| {
                let file = match tree.find(&file_id) {
                    Ok(file) => {
                        if file.owner() != root_owner {
                            return None;
                        }
                        file.clone()
                    }
                    _ => {
                        return None;
                    }
                };

                let file_size = match tree.calculate_deleted(&file_id).unwrap_or(true) {
                    true => 0,
                    false => file.file.timestamped_value.value.doc_size().unwrap_or(0),
                } as u64;

                Some(FileUsage { file_id, size_bytes: file_size + METADATA_FEE })
            })
            .collect();
        Ok(result)
    }

    pub fn get_usage_helper_v2<T>(
        _owner: &Owner, _tree: &mut LazyTree<T>,
    ) -> Result<Vec<FileUsage>, ServerError<GetUsageHelperError>>
    where
        T: TreeLike<F = ServerMeta>,
    {
        todo!()
    }

    pub fn get_cap(
        db: &ServerDb, public_key: &PublicKey,
    ) -> Result<u64, ServerError<GetUsageHelperError>> {
        Ok(db
            .accounts
            .get()
            .get(&Owner(*public_key))
            .ok_or(ServerError::ClientError(GetUsageHelperError::UserNotFound))?
            .billing_info
            .data_cap())
    }

    pub async fn delete_account(
        &self, context: RequestContext<DeleteAccountRequest>,
    ) -> Result<(), ServerError<DeleteAccountError>> {
        self.delete_account_helper(&context.public_key, false)
            .await?;

        Ok(())
    }

    pub async fn admin_disappear_account(
        &self, context: RequestContext<AdminDisappearAccountRequest>,
    ) -> Result<(), ServerError<AdminDisappearAccountError>> {
        let owner = {
            let db = &self.index_db.lock().await;

            if !Self::is_admin::<AdminDisappearAccountError>(
                db,
                &context.public_key,
                &self.config.admin.admins,
            )? {
                return Err(ClientError(AdminDisappearAccountError::NotPermissioned));
            }

            let admin_username = db
                .accounts
                .get()
                .get(&Owner(context.public_key))
                .cloned()
                .map(|account| account.username)
                .unwrap_or_else(|| "~unknown~".to_string());

            warn!("admin {} is disappearing account {}", admin_username, context.request.username);

            *db.usernames
                .get()
                .get(&context.request.username)
                .ok_or(ClientError(AdminDisappearAccountError::UserNotFound))?
        };

        self.delete_account_helper(&owner.0, true).await?;

        Ok(())
    }

    pub async fn admin_list_users(
        &self, context: RequestContext<AdminListUsersRequest>,
    ) -> Result<AdminListUsersResponse, ServerError<AdminListUsersError>> {
        let (db, request) = (&self.index_db.lock().await, &context.request);

        if !Self::is_admin::<AdminListUsersError>(
            db,
            &context.public_key,
            &self.config.admin.admins,
        )? {
            return Err(ClientError(AdminListUsersError::NotPermissioned));
        }

        let mut users: Vec<String> = vec![];

        for account in db.accounts.get().values() {
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
                    AccountFilter::GooglePlayPremium => match account.billing_info.billing_platform
                    {
                        Some(BillingPlatform::GooglePlay(_))
                            if account.billing_info.is_premium() =>
                        {
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
        &self, context: RequestContext<AdminGetAccountInfoRequest>,
    ) -> Result<AdminGetAccountInfoResponse, ServerError<AdminGetAccountInfoError>> {
        let (mut lock, request) = (self.index_db.lock().await, &context.request);
        let db = lock.deref_mut();

        if !Self::is_admin::<AdminGetAccountInfoError>(
            db,
            &context.public_key,
            &self.config.admin.admins,
        )? {
            return Err(ClientError(AdminGetAccountInfoError::NotPermissioned));
        }

        let owner = match &request.identifier {
            AccountIdentifier::PublicKey(public_key) => Owner(*public_key),
            AccountIdentifier::Username(user) => *db
                .usernames
                .get()
                .get(user)
                .ok_or(ClientError(AdminGetAccountInfoError::UserNotFound))?,
        };

        let account = db
            .accounts
            .get()
            .get(&owner)
            .ok_or(ClientError(AdminGetAccountInfoError::UserNotFound))?
            .clone();

        let mut maybe_root = None;
        if let Some(owned_ids) = db.owned_files.get().get(&owner) {
            for id in owned_ids {
                if let Some(meta) = db.metas.get().get(id) {
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

        let usage: u64 = Self::get_usage_helper(&mut tree)
            .map_err(|err| {
                internal!("Cannot find user's usage, owner: {:?}, err: {:?}", owner, err)
            })?
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

    pub async fn delete_account_helper(
        &self, public_key: &PublicKey, free_username: bool,
    ) -> Result<(), ServerError<DeleteAccountHelperError>> {
        let mut docs_to_delete = Vec::new();

        {
            let mut lock = self.index_db.lock().await;
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
            let metas_to_delete = tree.ids();

            for id in metas_to_delete.clone() {
                if !tree.calculate_deleted(&id)? {
                    let meta = tree.find(&id)?;
                    if meta.is_document() && &(meta.owner().0) == public_key {
                        if let Some(hmac) = meta.document_hmac() {
                            docs_to_delete.push((*meta.id(), *hmac));
                        }
                    }
                }
            }
            db.owned_files.clear_key(&Owner(*public_key))?;
            db.shared_files.clear_key(&Owner(*public_key))?;
            db.last_seen.remove(&Owner(*public_key))?;

            for id in metas_to_delete {
                if let Some(meta) = db.metas.get().get(&id) {
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
            self.document_service.delete(&id, &version).await?;
        }
        Ok(())
    }

    pub fn is_admin<E: Debug>(
        db: &ServerDb, public_key: &PublicKey, admins: &HashSet<Username>,
    ) -> Result<bool, ServerError<E>> {
        let is_admin = match db.accounts.get().get(&Owner(*public_key)) {
            None => false,
            Some(account) => admins.contains(&account.username),
        };

        Ok(is_admin)
    }
}

#[derive(Debug)]
pub enum GetUsageHelperError {
    UserNotFound,
    UserDeleted,
}

#[derive(Debug)]
pub enum DeleteAccountHelperError {
    UserNotFound,
}
