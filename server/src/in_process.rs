//! The server as a library inside a client's process. It trusts its caller: requests carry no
//! signature, and the account's public key stands in for the one a signature would prove.

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use db_rs::View;
use lb_rs::io::network::ApiError;
use lb_rs::io::server::{ApiResult, Server};
use lb_rs::model::account::Account;
use lb_rs::model::api::*;
use lb_rs::model::crypto::EncryptedDocument;
use lb_rs::model::file_metadata::DocumentHmac;
use uuid::Uuid;

use crate::billing::Nop;
use crate::billing::app_store_client::AppStoreClient;
use crate::billing::google_play_client::GooglePlayClient;
use crate::billing::stripe_client::StripeClient;
use crate::config::{
    AdminConfig, AppleConfig, BillingConfig, Config, Environment, FeatureFlags, FilesConfig,
    GoogleConfig, IndexDbConf, MetricsConfig, ServerConfig, StripeConfig,
};
use crate::document_service::DocumentService;
use crate::schema::ServerDb;
use crate::utils::get_build_info;
use crate::{RequestContext, ServerError, ServerState};

/// A server whose index and documents live in memory, with no billing.
pub type InMemoryServer = ServerState<Nop, Nop, Nop, InMemDocuments>;

impl InMemoryServer {
    /// An empty server that accepts new accounts and has no admins.
    pub fn in_memory() -> Self {
        let config = in_process_config();
        let index_db = ServerDb::init(&db_rs::config::Config::in_memory())
            .expect("an in-memory database initializes");
        Self {
            config,
            index_db: Arc::new(tokio::sync::Mutex::new(index_db)),
            stripe_client: Nop {},
            google_play_client: Nop {},
            app_store_client: Nop {},
            document_service: InMemDocuments::default(),
            discord_client: Default::default(),
            recent_new_account_ips: Default::default(),
            clock: Default::default(),
        }
    }
}

/// Every setting a handler reads, chosen for tests: accounts open, no rate limit, no admins.
/// Settings the in-process path never reads are empty.
fn in_process_config() -> Config {
    let none = String::new;
    Config {
        server: ServerConfig {
            env: Environment::Local,
            port: 0,
            max_auth_delay: 0,
            log_path: none(),
            pd_api_key: None,
            discord_webhook_url: None,
            ssl_cert_location: None,
            ssl_private_key_location: None,
            min_core_version: semver::VersionReq::STAR,
        },
        index_db: IndexDbConf { db_location: none(), time_between_compacts: Duration::ZERO },
        files: FilesConfig { path: PathBuf::new() },
        metrics: MetricsConfig {
            addr: ([127, 0, 0, 1], 0).into(),
            time_between_metrics_refresh: Duration::ZERO,
            time_between_metrics: Duration::ZERO,
        },
        billing: BillingConfig {
            millis_between_user_payment_flows: 0,
            time_between_lock_attempts: Duration::ZERO,
            google: GoogleConfig {
                service_account_key: None,
                premium_subscription_product_id: none(),
                premium_subscription_offer_id: none(),
                pubsub_token: none(),
            },
            stripe: StripeConfig {
                stripe_secret: none(),
                signing_secret: none(),
                premium_price_id: none(),
                premium_yearly_price_id: none(),
            },
            apple: AppleConfig {
                iap_key: none(),
                iap_key_id: none(),
                asc_public_key: none(),
                issuer_id: none(),
                subscription_product_id: none(),
                asc_shared_secret: none(),
                apple_root_cert: vec![],
                monthly_sub_group_id: none(),
            },
        },
        admin: AdminConfig { admins: HashSet::new() },
        features: FeatureFlags {
            new_accounts: true,
            new_account_rate_limit: false,
            bandwidth_controls: true,
        },
    }
}

/// Documents held in memory.
#[derive(Clone, Default)]
pub struct InMemDocuments {
    docs: Arc<Mutex<HashMap<(Uuid, DocumentHmac), EncryptedDocument>>>,
}

#[async_trait]
impl DocumentService for InMemDocuments {
    async fn insert<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac, content: &EncryptedDocument,
    ) -> Result<(), ServerError<T>> {
        self.docs
            .lock()
            .unwrap()
            .insert((*id, *hmac), content.clone());
        Ok(())
    }

    async fn get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<EncryptedDocument, ServerError<T>> {
        self.maybe_get(id, hmac)
            .await?
            .ok_or_else(|| internal!("no document {id} at that hmac"))
    }

    async fn maybe_get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<Option<EncryptedDocument>, ServerError<T>> {
        Ok(self.docs.lock().unwrap().get(&(*id, *hmac)).cloned())
    }

    async fn delete<T: Debug>(&self, id: &Uuid, hmac: &DocumentHmac) -> Result<(), ServerError<T>> {
        self.docs.lock().unwrap().remove(&(*id, *hmac));
        Ok(())
    }

    fn exists(&self, id: &Uuid, hmac: &DocumentHmac) -> bool {
        self.docs.lock().unwrap().contains_key(&(*id, *hmac))
    }

    fn get_path(&self, id: &Uuid, hmac: &DocumentHmac) -> PathBuf {
        let hmac = base64::encode_config(hmac, base64::URL_SAFE);
        PathBuf::from(format!("memory/{id}-{hmac}"))
    }
}

fn context<T>(account: &Account, request: T) -> RequestContext<T> {
    RequestContext { request, public_key: account.public_key(), ip: None }
}

fn api_error<E: Debug>(err: ServerError<E>) -> ApiError<E> {
    match err {
        ServerError::ClientError(e) => ApiError::Endpoint(e),
        ServerError::InternalError(e) => {
            tracing::error!("internal: {e}");
            ApiError::InternalError
        }
    }
}

impl<S, A, G, D> Server for ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    async fn new_account_v2(
        &self, account: &Account, request: NewAccountRequestV2,
    ) -> ApiResult<NewAccountRequestV2> {
        ServerState::new_account_v2(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn change_doc_v2(
        &self, account: &Account, request: ChangeDocRequestV2,
    ) -> ApiResult<ChangeDocRequestV2> {
        ServerState::change_doc_v2(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn upsert_file_metadata_v2(
        &self, account: &Account, request: UpsertRequestV2,
    ) -> ApiResult<UpsertRequestV2> {
        ServerState::upsert_file_metadata_v2(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn get_document(
        &self, account: &Account, request: GetDocRequest,
    ) -> ApiResult<GetDocRequest> {
        ServerState::get_document(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn get_public_key(
        &self, account: &Account, request: GetPublicKeyRequest,
    ) -> ApiResult<GetPublicKeyRequest> {
        ServerState::get_public_key(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn get_username(
        &self, account: &Account, request: GetUsernameRequest,
    ) -> ApiResult<GetUsernameRequest> {
        ServerState::get_username(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn get_usage(
        &self, account: &Account, request: GetUsageRequest,
    ) -> ApiResult<GetUsageRequest> {
        ServerState::get_usage(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn get_file_ids(
        &self, account: &Account, request: GetFileIdsRequest,
    ) -> ApiResult<GetFileIdsRequest> {
        ServerState::get_file_ids(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn get_updates_v2(
        &self, account: &Account, request: GetUpdatesRequestV2,
    ) -> ApiResult<GetUpdatesRequestV2> {
        ServerState::get_updates_v2(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn get_build_info(
        &self, _: &Account, _: GetBuildInfoRequest,
    ) -> ApiResult<GetBuildInfoRequest> {
        get_build_info().map_err(api_error)
    }

    async fn delete_account(
        &self, account: &Account, request: DeleteAccountRequest,
    ) -> ApiResult<DeleteAccountRequest> {
        ServerState::delete_account(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn stripe_checkout(
        &self, account: &Account, request: StripeCheckoutRequest,
    ) -> ApiResult<StripeCheckoutRequest> {
        ServerState::stripe_checkout(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn stripe_billing_portal(
        &self, account: &Account, request: StripeBillingPortalRequest,
    ) -> ApiResult<StripeBillingPortalRequest> {
        ServerState::stripe_billing_portal(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn upgrade_account_google_play(
        &self, account: &Account, request: UpgradeAccountGooglePlayRequest,
    ) -> ApiResult<UpgradeAccountGooglePlayRequest> {
        ServerState::upgrade_account_google_play(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn upgrade_account_app_store(
        &self, account: &Account, request: UpgradeAccountAppStoreRequest,
    ) -> ApiResult<UpgradeAccountAppStoreRequest> {
        ServerState::upgrade_account_app_store(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn cancel_subscription(
        &self, account: &Account, request: CancelSubscriptionRequest,
    ) -> ApiResult<CancelSubscriptionRequest> {
        ServerState::cancel_subscription(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn get_subscription_info(
        &self, account: &Account, request: GetSubscriptionInfoRequest,
    ) -> ApiResult<GetSubscriptionInfoRequest> {
        ServerState::get_subscription_info(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn get_subscription_info_v2(
        &self, account: &Account, request: GetSubscriptionInfoRequestV2,
    ) -> ApiResult<GetSubscriptionInfoRequestV2> {
        ServerState::get_subscription_info_v2(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn upsert_debug_info(
        &self, account: &Account, request: UpsertDebugInfoRequest,
    ) -> ApiResult<UpsertDebugInfoRequest> {
        ServerState::upsert_debug_info(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn admin_disappear_account(
        &self, account: &Account, request: AdminDisappearAccountRequest,
    ) -> ApiResult<AdminDisappearAccountRequest> {
        ServerState::admin_disappear_account(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn admin_disappear_file(
        &self, account: &Account, request: AdminDisappearFileRequest,
    ) -> ApiResult<AdminDisappearFileRequest> {
        ServerState::admin_disappear_file(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn admin_validate_account(
        &self, account: &Account, request: AdminValidateAccountRequest,
    ) -> ApiResult<AdminValidateAccountRequest> {
        ServerState::admin_validate_account(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn admin_validate_server(
        &self, account: &Account, request: AdminValidateServerRequest,
    ) -> ApiResult<AdminValidateServerRequest> {
        ServerState::admin_validate_server(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn admin_list_users(
        &self, account: &Account, request: AdminListUsersRequest,
    ) -> ApiResult<AdminListUsersRequest> {
        ServerState::admin_list_users(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn admin_get_account_info(
        &self, account: &Account, request: AdminGetAccountInfoRequest,
    ) -> ApiResult<AdminGetAccountInfoRequest> {
        ServerState::admin_get_account_info(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn admin_file_info(
        &self, account: &Account, request: AdminFileInfoRequest,
    ) -> ApiResult<AdminFileInfoRequest> {
        ServerState::admin_file_info(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn admin_set_user_tier(
        &self, account: &Account, request: AdminSetUserTierRequest,
    ) -> ApiResult<AdminSetUserTierRequest> {
        ServerState::admin_set_user_tier(self, context(account, request))
            .await
            .map_err(api_error)
    }

    async fn admin_rebuild_index(
        &self, account: &Account, request: AdminRebuildIndexRequest,
    ) -> ApiResult<AdminRebuildIndexRequest> {
        ServerState::admin_rebuild_index(self, context(account, request))
            .await
            .map_err(api_error)
    }
}
