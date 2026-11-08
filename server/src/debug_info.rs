use crate::ServerError::ClientError;
use crate::guard::ServerTx;
use std::fmt::Debug;

use lb_rs::model::{
    account::BETA_USERS,
    api::{UpsertDebugInfoError, UpsertDebugInfoRequest},
    file_metadata::Owner,
};
use lb_rs::service::debug::DebugInfo;
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
    schema::ServerV6,
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

        let mut tx = ServerTx::begin(&mut lock)?;
        let db = &mut *tx;

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

        tx.end()?;

        Ok(())
    }

    async fn send_panic_to_discord(
        &self, debug_info: &DebugInfo, panic: &str,
    ) -> Result<(), ServerError<UpsertDebugInfoError>> {
        let discord_webhook_url = match &self.config.server.discord_webhook_url {
            Some(url) => url,
            None => return Ok(()),
        };

        let panic_title: String = panic
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(256)
            .collect();

        let payload = json!({
            "username": "Panic Reporter",
            "embeds": [{
                "color": 14622784,
                "author": { "name": debug_info.name },
                "title": panic_title,
            }]
        });

        let debug_info_part =
            multipart::Part::bytes(panic_report_markdown(debug_info)?.into_bytes())
                .file_name(format!("{}_{}_debug_info.md", debug_info.name, debug_info.panics.len()))
                .mime_str("text/markdown")?;

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

    pub fn is_beta_user<E: Debug>(db: &ServerV6, public_key: &PublicKey) -> bool {
        let is_beta = match db.accounts.get(&Owner(*public_key)) {
            None => false,
            Some(account) => BETA_USERS.contains(&account.username.as_str()),
        };

        is_beta
    }
}

fn panic_report_markdown(debug_info: &DebugInfo) -> Result<String, serde_json::Error> {
    let mut summary = serde_json::to_value(debug_info)?;
    if let Some(fields) = summary.as_object_mut() {
        fields.remove("panics");
    }

    let mut report = format!(
        "# {}'s current debug info\n\n```json\n{}\n```\n",
        debug_info.name,
        serde_json::to_string_pretty(&summary)?
    );

    let count = debug_info.panics.len();
    for (index, panic) in debug_info.panics.iter().enumerate() {
        let panic = panic.trim();
        let (panic, config) = match panic.rsplit_once("\n\n") {
            Some((rest, config)) if config.starts_with('{') => (rest.trim_end(), Some(config)),
            _ => (panic, None),
        };
        let title = panic.lines().next().unwrap_or_default();

        report += &format!("\n# panic {} - {title}\n\n```\n{panic}\n```\n", count - index);
        if let Some(config) = config {
            report += &format!("\n```json\n{config}\n```\n");
        }
    }

    Ok(report)
}
