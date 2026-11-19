use crate::ServerError::ClientError;
use crate::billing::app_store_client::AppStoreClient;
use crate::billing::google_play_client::GooglePlayClient;
use crate::billing::stripe_client::StripeClient;
use crate::document_service::DocumentService;
use crate::guard::ServerTx;
use crate::schema::{Account, ServerV6, SubAccount};
use crate::utils::sub_account_username_is_valid;
use crate::{RequestContext, ServerError, ServerState};
use lb_rs::model::api::{
    DeleteSubAccountError, DeleteSubAccountRequest, ListSubAccountsError, ListSubAccountsRequest,
    ListSubAccountsResponse, NewSubAccountError, NewSubAccountRequest, SubAccountInfo,
};
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::Owner;
use lb_rs::model::pubkey;
use lb_rs::model::server_meta::IntoServerMeta;
use lb_rs::model::server_tree::ServerTree;
use lb_rs::model::tree_like::TreeLike;
use libsecp256k1::PublicKey;
use std::fmt::Debug;

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub async fn new_sub_account(
        &self, mut context: RequestContext<NewSubAccountRequest>,
    ) -> Result<(), ServerError<NewSubAccountError>> {
        use NewSubAccountError::*;
        context.request.username = context.request.username.to_lowercase();
        let request = &context.request;
        let parent = Owner(context.public_key);
        let owner = Owner(request.public_key);

        tracing::info!("new-sub-account attempt username: {}", request.username);

        if !self.config.features.new_accounts {
            return Err(ClientError(Disabled));
        }

        let mut guard = self.index_db.lock().await;
        let mut tx = ServerTx::begin(&mut guard)?;
        let db = &mut *tx;

        if let Some(ip) = context.ip {
            if !self.can_create_account(ip.ip()).await {
                return Err(ClientError(RateLimited));
            }
        }

        let parent_account = db
            .accounts
            .get(&parent)
            .ok_or(ClientError(NotPermissioned))?;
        let parent_is_deleted = db.owned_files.get(&parent).is_none_or(|ids| ids.is_empty());
        if parent_is_deleted || db.sub_accounts.contains_key(&parent) {
            return Err(ClientError(NotPermissioned));
        }
        if !parent_account.billing_info.is_premium() {
            return Err(ClientError(NotPremium));
        }
        if !sub_account_username_is_valid(&parent_account.username, &request.username) {
            return Err(ClientError(InvalidUsername));
        }

        let counter = Self::next_counter(db, parent);
        if request.counter != counter {
            return Err(ClientError(CounterIncorrect));
        }

        let root = &request.root_folder;
        let signed_at = root.timestamped_value.timestamp;
        if !root.is_root()
            || root.owner() != owner
            || pubkey::verify(&request.public_key, root, 0, 0, signed_at).is_err()
            || self.config.features.refuses(root)
        {
            return Err(ClientError(RootInvalid));
        }
        let root = root.clone().add_time(self.clock.now().0 as u64);

        if db.accounts.contains_key(&owner) {
            return Err(ClientError(PublicKeyTaken));
        }
        if db.usernames.contains_key(&request.username) {
            return Err(ClientError(UsernameTaken));
        }
        if db.metas.contains_key(root.id()) {
            return Err(ClientError(FileIdTaken));
        }

        if self.config.features.new_account_rate_limit {
            if let Some(ip) = context.ip {
                self.did_create_account(ip.ip()).await;
            }
        }

        let account =
            Account { username: request.username.clone(), billing_info: Default::default() };

        db.accounts.insert(owner, account)?;
        db.usernames.insert(request.username.clone(), owner)?;
        db.owned_files.insert(owner, *root.id())?;
        db.shared_files.create_key(owner)?;
        db.file_children.create_key(*root.id())?;
        db.metas.insert(*root.id(), root)?;
        db.sub_accounts
            .insert(owner, SubAccount { parent, counter, deleted: false })?;
        db.sub_accounts_by_parent.insert(parent, owner)?;

        tx.end()?;

        Ok(())
    }

    pub async fn list_sub_accounts(
        &self, context: RequestContext<ListSubAccountsRequest>,
    ) -> Result<ListSubAccountsResponse, ServerError<ListSubAccountsError>> {
        let parent = Owner(context.public_key);

        let mut guard = self.index_db.lock().await;
        let mut tx = ServerTx::begin(&mut guard)?;
        let db = &mut *tx;

        if !db.accounts.contains_key(&parent) {
            return Err(ClientError(ListSubAccountsError::UserNotFound));
        }

        let next_counter = Self::next_counter(db, parent);

        let mut sub_accounts = Vec::new();
        for owner in Self::live_sub_accounts(db, parent) {
            let (Some(account), Some(sub_account)) =
                (db.accounts.get(&owner), db.sub_accounts.get(&owner))
            else {
                continue;
            };
            let (username, counter) = (account.username.clone(), sub_account.counter);
            let usage = Self::usage(db, owner)?;
            sub_accounts.push(SubAccountInfo { username, public_key: owner.0, counter, usage });
        }
        sub_accounts.sort_by_key(|sub_account| sub_account.counter);

        tx.end()?;

        Ok(ListSubAccountsResponse { sub_accounts, next_counter })
    }

    pub async fn delete_sub_account(
        &self, context: RequestContext<DeleteSubAccountRequest>,
    ) -> Result<(), ServerError<DeleteSubAccountError>> {
        let parent = Owner(context.public_key);
        let owner = {
            let db = &self.index_db.lock().await.schema;
            db.usernames
                .get(&context.request.username.to_lowercase())
                .copied()
                .filter(|owner| Self::live_sub_accounts(db, parent).contains(owner))
                .ok_or(ClientError(DeleteSubAccountError::UserNotFound))?
        };

        self.delete_account_helper(&owner.0, false).await?;

        Ok(())
    }

    pub async fn is_sub_account(&self, public_key: &PublicKey) -> bool {
        self.index_db
            .lock()
            .await
            .schema
            .sub_accounts
            .contains_key(&Owner(*public_key))
    }

    pub fn billed_owner(db: &ServerV6, owner: Owner) -> Owner {
        db.sub_accounts
            .get(&owner)
            .map(|sub_account| sub_account.parent)
            .unwrap_or(owner)
    }

    pub fn family(db: &ServerV6, owner: Owner) -> Vec<Owner> {
        let parent = Self::billed_owner(db, owner);
        let mut family = vec![parent];
        family.extend(Self::live_sub_accounts(db, parent));
        family
    }

    fn sub_accounts_of(db: &ServerV6, parent: Owner) -> impl Iterator<Item = (Owner, &SubAccount)> {
        db.sub_accounts_by_parent
            .get(&parent)
            .into_iter()
            .flatten()
            .filter_map(|owner| Some((*owner, db.sub_accounts.get(owner)?)))
    }

    pub fn live_sub_accounts(db: &ServerV6, parent: Owner) -> Vec<Owner> {
        Self::sub_accounts_of(db, parent)
            .filter(|(_, sub_account)| !sub_account.deleted)
            .map(|(owner, _)| owner)
            .collect()
    }

    fn next_counter(db: &ServerV6, parent: Owner) -> u32 {
        Self::sub_accounts_of(db, parent)
            .map(|(_, sub_account)| sub_account.counter + 1)
            .max()
            .unwrap_or_default()
    }

    pub fn usage<E: Debug>(db: &mut ServerV6, owner: Owner) -> Result<u64, ServerError<E>> {
        ServerTree::new(
            owner,
            &mut db.owned_files,
            &mut db.shared_files,
            &mut db.file_children,
            &mut db.file_attachments,
            &mut db.metas,
        )
        .and_then(|tree| tree.to_lazy().calculate_usage(owner))
        .map_err(|err| internal!("{:?}", err))
    }

    pub fn other_family_usage<E: Debug>(
        db: &mut ServerV6, owner: Owner,
    ) -> Result<u64, ServerError<E>> {
        let mut usage = 0;
        for member in Self::family(db, owner) {
            if member != owner {
                usage += Self::usage(db, member)?;
            }
        }
        Ok(usage)
    }
}
