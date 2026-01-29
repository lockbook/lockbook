use crate::ServerError::ClientError;
use std::fmt::Debug;
use std::ops::DerefMut;

use db_rs::Db;
use lb_rs::model::{
    account::BETA_USERS,
    api::{UpsertDebugInfoError, UpsertDebugInfoRequest},
    file_metadata::Owner,
};
use libsecp256k1::PublicKey;
use reqwest::multipart;
use serde_json::json;
use tracing::{info, warn};

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

        if !Self::is_beta_user::<UpsertDebugInfoError>(db, &context.public_key) {
            return Err(ClientError(UpsertDebugInfoError::NotPermissioned));
        }

        let owner = Owner(context.public_key);

        let debug_info = context.request.debug_info.clone();
        let new_panics_count = debug_info.panics.len();

        let maybe_old_debug_info = db.debug_info.insert(
            owner,
            context.request.debug_info.lb_id,
            context.request.debug_info,
        )?;

        let old_panics_count =
            if let Some(debug_info) = maybe_old_debug_info { debug_info.panics.len() } else { 0 };

        if new_panics_count > old_panics_count {
            if let Some(panic) = debug_info.panics.first() {
                warn!(?debug_info, "beta user experienced a panic");

                self.send_panic_to_discord(&debug_info, panic).await?;
            }
        }

        tx.drop_safely()?;

        Ok(())
    }

    async fn send_panic_to_discord(
        &self, debug_info: &lb_rs::service::debug::DebugInfo, panic: &str,
    ) -> Result<(), ServerError<UpsertDebugInfoError>> {
        let discord_webhook_url = match &self.config.server.discord_webhook_url {
            Some(url) => url,
            None => return Ok(()),
        };

        let maybe_new_line_index = panic.find('\n');
        let mut panic_title = None;

        if let Some(new_line_index) = maybe_new_line_index {
            panic_title = Some(panic[0..new_line_index].to_string());
        }

        let payload = json!({
            "username": "Panic Reporter",
            "embeds": [{
                "color": 14622784,
                "author": { "name": debug_info.name },
                "title": panic_title.unwrap_or("".to_string()),
            }]
        });

        let debug_info_part = multipart::Part::bytes(serde_json::to_vec_pretty(&debug_info)?)
            .file_name(format!("{}_{}_debug_info.json", debug_info.name, debug_info.panics.len()))
            .mime_str("application/json")?;

        let form = multipart::Form::new()
            .part("file", debug_info_part)
            .text("payload_json", payload.to_string());

        let response = self
            .discord_client
            .post(discord_webhook_url)
            .multipart(form)
            .send()
            .await?;

        if response.status().is_success() {
            info!("Notifed discord of a panic!");
        } else {
            warn!("Failed to notify discord: {:?} {:?}", response.status(), response.text().await?);
        }
        Ok(())
    }

    pub fn is_beta_user<E: Debug>(db: &ServerDb, public_key: &PublicKey) -> bool {
        let is_beta = match db.accounts.get().get(&Owner(*public_key)) {
            None => false,
            Some(account) => BETA_USERS.contains(&account.username.as_str()),
        };

        is_beta
    }
}
