//! The library that underlies most things [lockbook](https://lockbook.net).
//!
//! All lockbook clients
//! (iOS, linux, etc) rely on this library to perform cryptography, offline edits, and
//! reconciliation of data between our server, other clients, and other devices.
//!
//! Our server relies on this library for checking signatures, and validating whether tree
//! modifications are valid / authorized.
//!
//! - Most clients / integrators will be interested in the functions attached to the [Lb] struct.
//!   See the [service] module for evolving this functionality.
//! - The [model] module contains the specification of our data structures and contracts between
//!   components.
//! - The [blocking] module contains blocking variants of all [Lb] functions for consumers without
//!   async runtimes.
//! - The [io] module contains interactions with disk and network.

#[macro_use]
extern crate tracing;

pub mod blocking;
pub mod io;
pub mod ipc;
pub mod macros;
pub mod model;
pub mod search;
pub mod service;
pub mod subscribers;
#[cfg(target_family = "wasm")]
pub mod wasm;

#[derive(Clone)]
pub struct Lb {
    pub local: Arc<OnceLock<LocalLb>>,
    pub remote: Option<Arc<RemoteLb>>,
    pub config: Config,
}

#[derive(Clone)]
pub struct LocalLb {
    pub config: Config,
    pub user_last_seen: Arc<RwLock<Instant>>,
    pub keychain: Keychain,
    pub db: LbDb,
    pub docs: AsyncDocs,
    pub client: Network,
    pub events: EventSubs,
    pub status: StatusUpdater,
    pub syncer: Syncer,
    #[cfg(not(target_family = "wasm"))]
    pub search: SearchIndex,
}

impl LocalLb {
    #[instrument(level = "info", skip_all, err(Debug))]
    pub async fn init(config: Config) -> LbResult<Self> {
        let docs = AsyncDocs::from(&config);
        let db_cfg = db_rs::Config::in_folder(&config.writeable_path);
        // an flock held across iOS suspend causes 0xdead10cc, iOS has no IPC yet
        #[cfg(target_os = "ios")]
        let db_cfg = db_rs::Config { fs_locks: false, ..db_cfg };
        let db = CoreDb::init(db_cfg).map_err(|err| LbErrKind::Unexpected(format!("{err:#?}")))?;
        let keychain = Keychain::from(db.account.get());
        let db = Arc::new(RwLock::new(db));
        let client = Network::default();
        #[cfg(not(target_family = "wasm"))]
        let search = SearchIndex::default();

        let status = StatusUpdater::default();
        let syncer = Default::default();
        let events = EventSubs::default();
        let user_last_seen = Arc::new(RwLock::new(Instant::now()));

        let result = Self {
            config,
            keychain,
            db,
            docs,
            client,
            syncer,
            events,
            status,
            user_last_seen,
            #[cfg(not(target_family = "wasm"))]
            search,
        };

        #[cfg(not(target_family = "wasm"))]
        {
            result.setup_syncer();
            result.setup_search();
            result.setup_status().await?;
        }

        Ok(result)
    }
}

impl Lb {
    pub async fn init(config: Config) -> LbResult<Self> {
        let local: Arc<OnceLock<LocalLb>> = Arc::new(OnceLock::new());
        let init_err = match LocalLb::init(config.clone()).await {
            Ok(loc) => {
                logging::init(&loc.config)?;
                ipc::spawn_host(loc.clone());
                let _ = local.set(loc);
                return Ok(Self { local, remote: None, config });
            }
            Err(err) => err,
        };
        if let Some(remote) = ipc::connect_guest(&config).await {
            return Ok(Self { local, remote: Some(remote), config });
        }
        Err(init_err)
    }
}

impl Lb {
    pub async fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        if let Some(local) = self.local.get() {
            return local.create_account(username, api_url, welcome_doc).await;
        }
        let account = self
            .call::<Account>(Request::CreateAccount {
                username: username.to_string(),
                api_url: api_url.to_string(),
                welcome_doc,
            })
            .await?;
        self.cache_account_on_remote(&account);
        Ok(account)
    }

    pub async fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        if let Some(local) = self.local.get() {
            return local.import_account(key, api_url).await;
        }
        let account = self
            .call::<Account>(Request::ImportAccount {
                key: key.to_string(),
                api_url: api_url.map(|s| s.to_string()),
            })
            .await?;
        self.cache_account_on_remote(&account);
        Ok(account)
    }

    pub async fn import_account_private_key_v1(&self, account: Account) -> LbResult<Account> {
        if let Some(local) = self.local.get() {
            return local.import_account_private_key_v1(account).await;
        }
        let account = self
            .call::<Account>(Request::ImportAccountPrivateKeyV1 { account })
            .await?;
        self.cache_account_on_remote(&account);
        Ok(account)
    }

    pub async fn import_account_phrase(
        &self, phrase: [&str; 24], api_url: &str,
    ) -> LbResult<Account> {
        if let Some(local) = self.local.get() {
            return local.import_account_phrase(phrase, api_url).await;
        }
        let account = self
            .call::<Account>(Request::ImportAccountPhrase {
                phrase: phrase.iter().map(|s| s.to_string()).collect(),
                api_url: api_url.to_string(),
            })
            .await?;
        self.cache_account_on_remote(&account);
        Ok(account)
    }

    fn cache_account_on_remote(&self, account: &Account) {
        if let Some(remote) = &self.remote {
            remote.cache_account(account.clone());
        }
    }

    pub async fn delete_account(&self) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.delete_account().await;
        }
        self.call(Request::DeleteAccount).await
    }

    pub fn get_account(&self) -> LbResult<Account> {
        if let Some(local) = self.local.get() {
            return local.get_account().cloned();
        }
        self.remote
            .as_ref()
            .expect("get_account: remote must be set when local is unset")
            .get_account()
            .cloned()
    }

    pub async fn suggested_docs(&self, settings: RankingWeights) -> LbResult<Vec<Uuid>> {
        if let Some(local) = self.local.get() {
            return local.suggested_docs(settings).await;
        }
        self.call(Request::SuggestedDocs { settings }).await
    }

    pub async fn clear_suggested(&self) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.clear_suggested().await;
        }
        self.call(Request::ClearSuggested).await
    }

    pub async fn clear_suggested_id(&self, id: Uuid) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.clear_suggested_id(id).await;
        }
        self.call(Request::ClearSuggestedId { id }).await
    }

    pub fn app_foregrounded(&self) {
        if let Some(local) = self.local.get() {
            local.app_foregrounded();
            return;
        }
        if let Some(remote) = &self.remote {
            let r = Arc::clone(remote);
            tokio::spawn(async move {
                let _ = r.try_call::<()>(Request::AppForegrounded).await;
            });
        }
    }

    pub async fn disappear_account(&self, username: &str) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.disappear_account(username).await;
        }
        self.call(Request::DisappearAccount { username: username.to_string() })
            .await
    }

    pub async fn disappear_file(&self, id: Uuid) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.disappear_file(id).await;
        }
        self.call(Request::DisappearFile { id }).await
    }

    pub async fn list_users(&self, filter: Option<AccountFilter>) -> LbResult<Vec<Username>> {
        if let Some(local) = self.local.get() {
            return local.list_users(filter).await;
        }
        self.call(Request::ListUsers { filter }).await
    }

    pub async fn get_account_info(&self, identifier: AccountIdentifier) -> LbResult<AccountInfo> {
        if let Some(local) = self.local.get() {
            return local.get_account_info(identifier).await;
        }
        self.call(Request::GetAccountInfo { identifier }).await
    }

    pub async fn validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
        if let Some(local) = self.local.get() {
            return local.validate_account(username).await;
        }
        self.call(Request::AdminValidateAccount { username: username.to_string() })
            .await
    }

    pub async fn validate_server(&self) -> LbResult<AdminValidateServer> {
        if let Some(local) = self.local.get() {
            return local.validate_server().await;
        }
        self.call(Request::AdminValidateServer).await
    }

    pub async fn file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse> {
        if let Some(local) = self.local.get() {
            return local.file_info(id).await;
        }
        self.call(Request::AdminFileInfo { id }).await
    }

    pub async fn rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.rebuild_index(index).await;
        }
        self.call(Request::RebuildIndex { index }).await
    }

    pub async fn set_user_tier(&self, username: &str, info: AdminSetUserTierInfo) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.set_user_tier(username, info).await;
        }
        self.call(Request::SetUserTier { username: username.to_string(), info })
            .await
    }

    pub async fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.upgrade_account_stripe(account_tier).await;
        }
        self.call(Request::UpgradeAccountStripe { account_tier })
            .await
    }

    pub async fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local
                .upgrade_account_google_play(purchase_token, account_id)
                .await;
        }
        self.call(Request::UpgradeAccountGooglePlay {
            purchase_token: purchase_token.to_string(),
            account_id: account_id.to_string(),
        })
        .await
    }

    pub async fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local
                .upgrade_account_app_store(original_transaction_id, app_account_token)
                .await;
        }
        self.call(Request::UpgradeAccountAppStore { original_transaction_id, app_account_token })
            .await
    }

    pub async fn cancel_subscription(&self) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.cancel_subscription().await;
        }
        self.call(Request::CancelSubscription).await
    }

    pub async fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>> {
        if let Some(local) = self.local.get() {
            return local.get_subscription_info().await;
        }
        self.call(Request::GetSubscriptionInfo).await
    }

    #[cfg(not(target_family = "wasm"))]
    pub async fn recent_panic(&self) -> LbResult<bool> {
        if let Some(local) = self.local.get() {
            return local.recent_panic().await;
        }
        self.call(Request::RecentPanic).await
    }

    #[cfg(not(target_family = "wasm"))]
    pub async fn write_panic_to_file(&self, error_header: String, bt: String) -> LbResult<String> {
        if let Some(local) = self.local.get() {
            return local.write_panic_to_file(error_header, bt).await;
        }
        self.call(Request::WritePanicToFile { error_header, bt })
            .await
    }

    #[cfg(not(target_family = "wasm"))]
    pub async fn debug_info(&self, os_info: String, check_docs: bool) -> LbResult<DebugInfo> {
        if let Some(local) = self.local.get() {
            return local.debug_info(os_info, check_docs).await;
        }
        self.call(Request::DebugInfo { os_info, check_docs }).await
    }

    pub async fn read_document(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<DecryptedDocument> {
        if let Some(local) = self.local.get() {
            return local.read_document(id, user_activity).await;
        }
        self.call(Request::ReadDocument { id, user_activity }).await
    }

    pub async fn write_document(&self, id: Uuid, content: &[u8]) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.write_document(id, content).await;
        }
        self.call(Request::WriteDocument { id, content: content.to_vec() })
            .await
    }

    pub async fn read_document_with_hmac(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        if let Some(local) = self.local.get() {
            return local.read_document_with_hmac(id, user_activity).await;
        }
        self.call(Request::ReadDocumentWithHmac { id, user_activity })
            .await
    }

    pub async fn safe_write(
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        if let Some(local) = self.local.get() {
            return local.safe_write(id, old_hmac, content).await;
        }
        self.call(Request::SafeWrite { id, old_hmac, content })
            .await
    }

    pub async fn create_file(
        &self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> LbResult<File> {
        if let Some(local) = self.local.get() {
            return local.create_file(name, parent, file_type).await;
        }
        self.call::<File>(Request::CreateFile {
            name: name.to_string(),
            parent: *parent,
            file_type,
        })
        .await
    }

    pub async fn rename_file(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.rename_file(id, new_name).await;
        }
        self.call(Request::RenameFile { id: *id, new_name: new_name.to_string() })
            .await
    }

    pub async fn move_file(&self, id: &Uuid, new_parent: &Uuid) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.move_file(id, new_parent).await;
        }
        self.call(Request::MoveFile { id: *id, new_parent: *new_parent })
            .await
    }

    pub async fn delete(&self, id: &Uuid) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.delete(id).await;
        }
        self.call(Request::Delete { id: *id }).await
    }

    pub async fn root(&self) -> LbResult<File> {
        if let Some(local) = self.local.get() {
            return local.root().await;
        }
        self.call(Request::Root).await
    }

    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        if let Some(local) = self.local.get() {
            return local.list_metadatas().await;
        }
        self.call(Request::ListMetadatas).await
    }

    pub async fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        if let Some(local) = self.local.get() {
            return local.get_children(id).await;
        }
        self.call(Request::GetChildren { id: *id }).await
    }

    pub async fn get_and_get_children_recursively(&self, id: &Uuid) -> LbResult<Vec<File>> {
        if let Some(local) = self.local.get() {
            return local.get_and_get_children_recursively(id).await;
        }
        self.call(Request::GetAndGetChildrenRecursively { id: *id })
            .await
    }

    pub async fn get_file_by_id(&self, id: Uuid) -> LbResult<File> {
        if let Some(local) = self.local.get() {
            return local.get_file_by_id(id).await;
        }
        self.call(Request::GetFileById { id }).await
    }

    pub async fn get_file_link_url(&self, id: Uuid) -> LbResult<String> {
        if let Some(local) = self.local.get() {
            return local.get_file_link_url(id).await;
        }
        self.call(Request::GetFileLinkUrl { id }).await
    }

    pub async fn local_changes(&self) -> Vec<Uuid> {
        if let Some(local) = self.local.get() {
            return local.local_changes().await;
        }
        self.call::<_>(Request::LocalChanges)
            .await
            .unwrap_or_default()
    }

    pub async fn test_repo_integrity(&self, check_docs: bool) -> LbResult<Vec<Warning>> {
        if let Some(local) = self.local.get() {
            return local.test_repo_integrity(check_docs).await;
        }
        self.call(Request::TestRepoIntegrity { check_docs }).await
    }

    pub async fn create_link_at_path(&self, path: &str, target_id: Uuid) -> LbResult<File> {
        if let Some(local) = self.local.get() {
            return local.create_link_at_path(path, target_id).await;
        }
        self.call(Request::CreateLinkAtPath { path: path.to_string(), target_id })
            .await
    }

    pub async fn create_at_path(&self, path: &str) -> LbResult<File> {
        if let Some(local) = self.local.get() {
            return local.create_at_path(path).await;
        }
        self.call(Request::CreateAtPath { path: path.to_string() })
            .await
    }

    pub async fn get_by_path(&self, path: &str) -> LbResult<File> {
        if let Some(local) = self.local.get() {
            return local.get_by_path(path).await;
        }
        self.call(Request::GetByPath { path: path.to_string() })
            .await
    }

    pub async fn get_path_by_id(&self, id: Uuid) -> LbResult<String> {
        if let Some(local) = self.local.get() {
            return local.get_path_by_id(id).await;
        }
        self.call(Request::GetPathById { id }).await
    }

    pub async fn list_paths(&self, filter: Option<Filter>) -> LbResult<Vec<String>> {
        if let Some(local) = self.local.get() {
            return local.list_paths(filter).await;
        }
        self.call(Request::ListPaths { filter }).await
    }

    pub async fn list_paths_with_ids(
        &self, filter: Option<Filter>,
    ) -> LbResult<Vec<(Uuid, String)>> {
        if let Some(local) = self.local.get() {
            return local.list_paths_with_ids(filter).await;
        }
        self.call(Request::ListPathsWithIds { filter }).await
    }

    pub async fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.share_file(id, username, mode).await;
        }
        self.call(Request::ShareFile { id, username: username.to_string(), mode })
            .await
    }

    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>> {
        if let Some(local) = self.local.get() {
            return local.get_pending_shares().await;
        }
        self.call(Request::GetPendingShares).await
    }

    pub async fn get_pending_share_files(&self) -> LbResult<Vec<File>> {
        if let Some(local) = self.local.get() {
            return local.get_pending_share_files().await;
        }
        self.call(Request::GetPendingShareFiles).await
    }

    pub async fn known_usernames(&self) -> LbResult<Vec<String>> {
        if let Some(local) = self.local.get() {
            return local.known_usernames().await;
        }
        self.call(Request::KnownUsernames).await
    }

    pub async fn reject_share(&self, id: &Uuid) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.reject_share(id).await;
        }
        self.call(Request::RejectShare { id: *id }).await
    }

    pub async fn pin_file(&self, id: Uuid) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.pin_file(id).await;
        }
        self.call(Request::PinFile { id }).await
    }

    pub async fn unpin_file(&self, id: Uuid) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.unpin_file(id).await;
        }
        self.call(Request::UnpinFile { id }).await
    }

    pub async fn list_pinned(&self) -> LbResult<Vec<Uuid>> {
        if let Some(local) = self.local.get() {
            return local.list_pinned().await;
        }
        self.call(Request::ListPinned).await
    }

    pub async fn get_usage(&self) -> LbResult<UsageMetrics> {
        if let Some(local) = self.local.get() {
            return local.get_usage().await;
        }
        self.call(Request::GetUsage).await
    }

    pub async fn sync(&self) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.sync().await;
        }
        self.call(Request::Sync).await
    }

    pub async fn status(&self) -> Status {
        if let Some(local) = self.local.get() {
            return local.status().await;
        }
        self.call::<_>(Request::Status).await.unwrap_or_default()
    }

    pub async fn get_last_synced(&self) -> LbResult<i64> {
        if let Some(local) = self.local.get() {
            return local.get_last_synced().await;
        }
        self.call(Request::GetLastSynced).await
    }

    pub async fn get_last_synced_human(&self) -> LbResult<String> {
        if let Some(local) = self.local.get() {
            return local.get_last_synced_human().await;
        }
        self.call(Request::GetLastSyncedHuman).await
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<service::events::Event> {
        if let Some(local) = self.local.get() {
            return local.subscribe();
        }
        self.remote
            .as_ref()
            .expect("subscribe: remote must be set when local is unset")
            .subscribe()
    }

    pub fn get_timestamp_human_string(&self, timestamp: i64) -> String {
        use basic_human_duration::ChronoHumanDuration;
        if timestamp != 0 {
            time::Duration::milliseconds(crate::model::clock::get_time().0 - timestamp)
                .format_human()
                .to_string()
        } else {
            "never".to_string()
        }
    }

    #[cfg(not(target_family = "wasm"))]
    pub async fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<Vec<SearchResult>> {
        if let Some(local) = self.local.get() {
            return local.search(input, cfg).await;
        }
        self.call(Request::Search { input: input.to_string(), cfg })
            .await
    }

    #[cfg(not(target_family = "wasm"))]
    pub async fn build_index(&self) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.build_index().await;
        }
        self.call(Request::BuildIndex).await
    }

    #[cfg(not(target_family = "wasm"))]
    pub async fn reload_search_index(&self) -> LbResult<()> {
        if let Some(local) = self.local.get() {
            return local.reload_search_index();
        }
        self.call(Request::ReloadSearchIndex).await
    }
}
pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub static DEFAULT_API_LOCATION: &str = "https://app.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");

use crate::io::CoreDb;
use crate::ipc::client::RemoteLb;
use crate::subscribers::syncer::Syncer;
use db_rs::Db;
#[cfg(not(target_family = "wasm"))]
use subscribers::search::SearchIndex;

use crate::service::logging;
use io::LbDb;
use io::docs::AsyncDocs;
use io::network::Network;
use model::core_config::Config;
pub use model::errors::{LbErrKind, LbResult};
use service::events::EventSubs;
use service::keychain::Keychain;
use std::sync::{Arc, OnceLock};
use subscribers::status::StatusUpdater;
use tokio::sync::RwLock;
pub use uuid::Uuid;
use web_time::Instant;

use crate::ipc::protocol::Request;
use crate::model::account::{Account, Username};
use crate::model::api::{
    AccountFilter, AccountIdentifier, AccountInfo, AdminFileInfoResponse, AdminSetUserTierInfo,
    AdminValidateAccount, AdminValidateServer, ServerIndex, StripeAccountTier, SubscriptionInfo,
};
use crate::model::crypto::DecryptedDocument;
use crate::model::errors::Warning;
use crate::model::file::{File, ShareMode};
use crate::model::file_metadata::{DocumentHmac, FileType};
use crate::model::path_ops::Filter;
use crate::service::activity::RankingWeights;
#[cfg(not(target_family = "wasm"))]
use crate::service::debug::DebugInfo;
use crate::service::usage::UsageMetrics;
#[cfg(not(target_family = "wasm"))]
use crate::subscribers::search::{SearchConfig, SearchResult};
use crate::subscribers::status::Status;
