use crate::{ServerError::ClientError, debug_info};
use std::collections::HashSet;
use std::fmt::Debug;
use std::ops::DerefMut;

use db_rs::Db;
use lb_rs::model::{
    account::BETA_USERS,
    account::Username,
    api::{UpsertDebugInfoError, UpsertDebugInfoRequest},
    file_metadata::Owner,
};
use libsecp256k1::PublicKey;

use crate::{
    RequestContext, ServerError, ServerState,
    billing::{
        app_store_client::AppStoreClient, google_play_client::GooglePlayClient,
        stripe_client::StripeClient,
    },
    document_service::DocumentService,
    schema::ServerDb,
};

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub async fn upsert_debug_info(
        &self, context: RequestContext<UpsertDebugInfoRequest>,
    ) -> Result<(), ServerError<UpsertDebugInfoError>> {
        let mut lock = self.index_db.lock().await;

        let db = lock.deref_mut();

        let tx = db.begin_transaction()?;

        if !Self::is_beta_user::<UpsertDebugInfoError>(db, &context.public_key)? {
            return Err(ClientError(UpsertDebugInfoError::NotPermissioned));
        }

        let owner = Owner(context.public_key);

        let panics_count = context.request.debug_info.panics.len();

        let maybe_old_debug_info =
            db.debug_info
                .insert(owner, context.request.lb_id, context.request.debug_info)?;

        let old_panics_count =
            if let Some(debug_info) = maybe_old_debug_info { debug_info.panics.len() } else { 0 };

        tx.drop_safely()?;

        if panics_count > old_panics_count {
            // notify discord
        }

        Ok(())
    }

    pub fn is_beta_user<E: Debug>(
        db: &ServerDb, public_key: &PublicKey,
    ) -> Result<bool, ServerError<E>> {
        let is_debug = match db.accounts.get().get(&Owner(*public_key)) {
            None => false,
            Some(account) => BETA_USERS.contains(&account.username.as_str()),
        };

        Ok(is_debug)
    }
}
