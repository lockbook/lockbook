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
pub mod ipc;
pub mod io;
pub mod macros;
pub mod model;
pub mod service;
pub mod subscribers;

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
    /// this is dumb lb that will make the library compile for wasm but doesn't include
    /// any of the expected functionality. your files wouldn't be saved, sync wouldn't
    /// work, etc. for now this is useful for unblocking workspace on wasm
    #[cfg(target_family = "wasm")]
    pub fn init_dummy(config: Config) -> LbResult<Self> {
        let db = CoreDb::init(db_rs::Config {
            path: Default::default(),
            create_path: false,
            create_db: false,
            read_only: false,
            no_io: true,
            fs_locks: false,
            fs_locks_block: false,
            schema_name: Default::default(),
        })
        .map_err(|err| LbErrKind::Unexpected(format!("db rs creation failed: {:#?}", err)))?;
        let user_last_seen = Arc::new(RwLock::new(Instant::now()));

        Ok(Self {
            user_last_seen,
            config: config.clone(),
            keychain: Default::default(),
            db: Arc::new(RwLock::new(db)),
            docs: AsyncDocs::from(&config),
            client: Default::default(),
            syncer: Default::default(),
            events: Default::default(),
            status: Default::default(),
        })
    }
}

impl LocalLb {
    #[instrument(level = "info", skip_all, err(Debug))]
    pub async fn init(config: Config) -> LbResult<Self> {
        logging::init(&config)?;

        let docs = AsyncDocs::from(&config);
        let db_cfg = db_rs::Config::in_folder(&config.writeable_path);
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

/// Public-facing handle to lb-rs.
///
/// Wraps an `LbInner` that's either `Local(LocalLb)` — the in-process
/// implementation holding the db-rs filesystem lock — or `Remote(RemoteLb)`
/// — an IPC client forwarding calls to the host process that holds the lock.
///
/// `Lb::init` races for the lock: success means becoming a Host (and
/// spawning the IPC listener); failure means connecting to the socket as a
/// Guest.
///
/// # Method coverage
///
/// Stage 3 exposes explicit forwarders for a vertical slice of `LocalLb`'s
/// public methods (see the impl block below). Methods *not* yet in that
/// slice are still reachable via a temporary `Deref<Target = LocalLb>` —
/// but only in Local mode. A Guest calling an unported method panics. Stage
/// 4 ports the remaining methods and removes `Deref`.
#[derive(Clone)]
pub struct Lb {
    inner: LbInner,
}

#[derive(Clone)]
enum LbInner {
    Local(LocalLb),
    /// Only constructed on platforms where the guest can actually reach the
    /// host over UDS (see `Lb::init`'s `cfg(unix)` guest branch). On other
    /// platforms the variant exists but is never populated, so the forwarder
    /// arms below compile cleanly without per-arm cfgs.
    Remote(RemoteLb),
}

impl Lb {
    /// Construct an `Lb`, racing the db-rs filesystem lock.
    ///
    /// Strategy:
    /// 1. Try `LocalLb::init`. If that succeeds we hold the lock — spawn
    ///    the IPC listener and return `Local`.
    /// 2. If `LocalLb::init` fails **and** a socket already exists in the
    ///    db folder (suggesting another process is the host), retry a
    ///    short handful of connects as a Guest.
    /// 3. On all-fails, surface the original `LocalLb::init` error so a
    ///    genuinely corrupt / missing folder doesn't get masked as "can't
    ///    connect".
    pub async fn init(config: Config) -> LbResult<Self> {
        let init_err = match LocalLb::init(config.clone()).await {
            Ok(local) => {
                #[cfg(unix)]
                {
                    let socket = ipc::socket_path(&local.config.writeable_path);
                    match ipc::transport::listen(&socket).await {
                        Ok(listener) => {
                            let lb_for_server = Arc::new(local.clone());
                            tokio::spawn(ipc::server::serve(listener, lb_for_server));
                        }
                        Err(err) => {
                            // Not fatal — the host still works, guests just
                            // can't attach until the socket is available.
                            tracing::warn!(
                                ?err,
                                "failed to bind ipc listener; guests cannot attach"
                            );
                        }
                    }
                }
                return Ok(Lb { inner: LbInner::Local(local) });
            }
            Err(err) => err,
        };

        #[cfg(unix)]
        {
            let socket = ipc::socket_path(&config.writeable_path);
            if socket.exists() {
                if let Ok(remote) = connect_guest_with_retry(&socket, &config).await {
                    return Ok(Lb { inner: LbInner::Remote(remote) });
                }
            }
        }

        Err(init_err)
    }

    /// see [`LocalLb::init_dummy`]
    #[cfg(target_family = "wasm")]
    pub fn init_dummy(config: Config) -> LbResult<Self> {
        let local = LocalLb::init_dummy(config)?;
        Ok(Lb { inner: LbInner::Local(local) })
    }
}

// ---- Explicit forwarders for the public Lb surface ----------------------
//
// Each forwarder dispatches on `inner`: Local runs in-process; Remote sends
// a typed [`Request`] variant over IPC via `RemoteLb::call::<Out>(req)`.
// The `Request` enum's discriminant picks the host-side method to invoke
// and carries its arguments; the response comes back as a bincode-encoded
// `LbResult<Out>`. If the server wrote a different `Out` than the caller
// expects, bincode fails and the error surfaces as `LbErrKind::Unexpected`.
//
// `LbInner::Remote` is only constructed on `cfg(unix)` inside `Lb::init`,
// so on non-Unix platforms the Remote arms below are statically unreachable
// — `RemoteLb::call` has a stub impl on those targets for completeness.
//
// The following methods are *not* forwarded explicitly and remain
// reachable only through the `Deref` shim (Local-only; Remote panics):
//
//   - `get_account`, `export_account_private_key`, `export_account_phrase`,
//     `export_account_qr` — sync, return values that need the in-memory
//     account. A future pass should cache the account on the Guest at
//     connect time so these can stay sync without IPC.

impl Lb {
    // -- account ----------------------------------------------------------

    pub async fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        match &self.inner {
            LbInner::Local(l) => l.create_account(username, api_url, welcome_doc).await,
            LbInner::Remote(r) => {
                r.call(Request::CreateAccount {
                    username: username.to_string(),
                    api_url: api_url.to_string(),
                    welcome_doc,
                })
                .await
            }
        }
    }

    pub async fn import_account(
        &self, key: &str, api_url: Option<&str>,
    ) -> LbResult<Account> {
        match &self.inner {
            LbInner::Local(l) => l.import_account(key, api_url).await,
            LbInner::Remote(r) => {
                r.call(Request::ImportAccount {
                    key: key.to_string(),
                    api_url: api_url.map(|s| s.to_string()),
                })
                .await
            }
        }
    }

    pub async fn import_account_private_key_v1(&self, account: Account) -> LbResult<Account> {
        match &self.inner {
            LbInner::Local(l) => l.import_account_private_key_v1(account).await,
            LbInner::Remote(r) => r.call(Request::ImportAccountPrivateKeyV1 { account }).await,
        }
    }

    pub async fn import_account_phrase(
        &self, phrase: [&str; 24], api_url: &str,
    ) -> LbResult<Account> {
        match &self.inner {
            LbInner::Local(l) => l.import_account_phrase(phrase, api_url).await,
            LbInner::Remote(r) => {
                let phrase: [String; 24] = std::array::from_fn(|i| phrase[i].to_string());
                r.call(Request::ImportAccountPhrase { phrase, api_url: api_url.to_string() })
                    .await
            }
        }
    }

    pub async fn delete_account(&self) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.delete_account().await,
            LbInner::Remote(r) => r.call(Request::DeleteAccount).await,
        }
    }

    // -- activity ---------------------------------------------------------

    pub async fn suggested_docs(&self, settings: RankingWeights) -> LbResult<Vec<Uuid>> {
        match &self.inner {
            LbInner::Local(l) => l.suggested_docs(settings).await,
            LbInner::Remote(r) => r.call(Request::SuggestedDocs { settings }).await,
        }
    }

    pub async fn clear_suggested(&self) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.clear_suggested().await,
            LbInner::Remote(r) => r.call(Request::ClearSuggested).await,
        }
    }

    pub async fn clear_suggested_id(&self, id: Uuid) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.clear_suggested_id(id).await,
            LbInner::Remote(r) => r.call(Request::ClearSuggestedId { id }).await,
        }
    }

    /// Hint the host that the user is around. Sync — fire-and-forget for
    /// guests so the existing sync caller surface doesn't change.
    pub fn app_foregrounded(&self) {
        match &self.inner {
            LbInner::Local(l) => l.app_foregrounded(),
            LbInner::Remote(r) => {
                let r = r.clone();
                tokio::spawn(async move {
                    let _: LbResult<()> = r.call(Request::AppForegrounded).await;
                });
            }
        }
    }

    // -- admin ------------------------------------------------------------

    pub async fn disappear_account(&self, username: &str) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.disappear_account(username).await,
            LbInner::Remote(r) => {
                r.call(Request::DisappearAccount { username: username.to_string() }).await
            }
        }
    }

    pub async fn disappear_file(&self, id: Uuid) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.disappear_file(id).await,
            LbInner::Remote(r) => r.call(Request::DisappearFile { id }).await,
        }
    }

    pub async fn list_users(
        &self, filter: Option<AccountFilter>,
    ) -> LbResult<Vec<Username>> {
        match &self.inner {
            LbInner::Local(l) => l.list_users(filter).await,
            LbInner::Remote(r) => r.call(Request::ListUsers { filter }).await,
        }
    }

    pub async fn get_account_info(
        &self, identifier: AccountIdentifier,
    ) -> LbResult<AccountInfo> {
        match &self.inner {
            LbInner::Local(l) => l.get_account_info(identifier).await,
            LbInner::Remote(r) => r.call(Request::GetAccountInfo { identifier }).await,
        }
    }

    pub async fn validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
        match &self.inner {
            LbInner::Local(l) => l.validate_account(username).await,
            LbInner::Remote(r) => {
                r.call(Request::AdminValidateAccount { username: username.to_string() })
                    .await
            }
        }
    }

    pub async fn validate_server(&self) -> LbResult<AdminValidateServer> {
        match &self.inner {
            LbInner::Local(l) => l.validate_server().await,
            LbInner::Remote(r) => r.call(Request::AdminValidateServer).await,
        }
    }

    pub async fn file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse> {
        match &self.inner {
            LbInner::Local(l) => l.file_info(id).await,
            LbInner::Remote(r) => r.call(Request::AdminFileInfo { id }).await,
        }
    }

    pub async fn rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.rebuild_index(index).await,
            LbInner::Remote(r) => r.call(Request::RebuildIndex { index }).await,
        }
    }

    pub async fn set_user_tier(
        &self, username: &str, info: AdminSetUserTierInfo,
    ) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.set_user_tier(username, info).await,
            LbInner::Remote(r) => {
                r.call(Request::SetUserTier { username: username.to_string(), info }).await
            }
        }
    }

    // -- billing ----------------------------------------------------------

    pub async fn upgrade_account_stripe(
        &self, account_tier: StripeAccountTier,
    ) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.upgrade_account_stripe(account_tier).await,
            LbInner::Remote(r) => r.call(Request::UpgradeAccountStripe { account_tier }).await,
        }
    }

    pub async fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => {
                l.upgrade_account_google_play(purchase_token, account_id).await
            }
            LbInner::Remote(r) => {
                r.call(Request::UpgradeAccountGooglePlay {
                    purchase_token: purchase_token.to_string(),
                    account_id: account_id.to_string(),
                })
                .await
            }
        }
    }

    pub async fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => {
                l.upgrade_account_app_store(original_transaction_id, app_account_token)
                    .await
            }
            LbInner::Remote(r) => {
                r.call(Request::UpgradeAccountAppStore {
                    original_transaction_id,
                    app_account_token,
                })
                .await
            }
        }
    }

    pub async fn cancel_subscription(&self) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.cancel_subscription().await,
            LbInner::Remote(r) => r.call(Request::CancelSubscription).await,
        }
    }

    pub async fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>> {
        match &self.inner {
            LbInner::Local(l) => l.get_subscription_info().await,
            LbInner::Remote(r) => r.call(Request::GetSubscriptionInfo).await,
        }
    }

    // -- debug (cfg!=wasm) -----------------------------------------------

    #[cfg(not(target_family = "wasm"))]
    pub async fn recent_panic(&self) -> LbResult<bool> {
        match &self.inner {
            LbInner::Local(l) => l.recent_panic().await,
            LbInner::Remote(r) => r.call(Request::RecentPanic).await,
        }
    }

    #[cfg(not(target_family = "wasm"))]
    pub async fn write_panic_to_file(
        &self, error_header: String, bt: String,
    ) -> LbResult<String> {
        match &self.inner {
            LbInner::Local(l) => l.write_panic_to_file(error_header, bt).await,
            LbInner::Remote(r) => {
                r.call(Request::WritePanicToFile { error_header, bt }).await
            }
        }
    }

    #[cfg(not(target_family = "wasm"))]
    pub async fn debug_info(
        &self, os_info: String, check_docs: bool,
    ) -> LbResult<DebugInfo> {
        match &self.inner {
            LbInner::Local(l) => l.debug_info(os_info, check_docs).await,
            LbInner::Remote(r) => r.call(Request::DebugInfo { os_info, check_docs }).await,
        }
    }

    // -- documents --------------------------------------------------------

    pub async fn read_document(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<DecryptedDocument> {
        match &self.inner {
            LbInner::Local(l) => l.read_document(id, user_activity).await,
            LbInner::Remote(r) => r.call(Request::ReadDocument { id, user_activity }).await,
        }
    }

    pub async fn write_document(&self, id: Uuid, content: &[u8]) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.write_document(id, content).await,
            LbInner::Remote(r) => {
                r.call(Request::WriteDocument { id, content: content.to_vec() }).await
            }
        }
    }

    pub async fn read_document_with_hmac(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        match &self.inner {
            LbInner::Local(l) => l.read_document_with_hmac(id, user_activity).await,
            LbInner::Remote(r) => {
                r.call(Request::ReadDocumentWithHmac { id, user_activity }).await
            }
        }
    }

    pub async fn safe_write(
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        match &self.inner {
            LbInner::Local(l) => l.safe_write(id, old_hmac, content).await,
            LbInner::Remote(r) => r.call(Request::SafeWrite { id, old_hmac, content }).await,
        }
    }

    // -- file -------------------------------------------------------------

    pub async fn create_file(
        &self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.create_file(name, parent, file_type).await,
            LbInner::Remote(r) => {
                r.call(Request::CreateFile {
                    name: name.to_string(),
                    parent: *parent,
                    file_type,
                })
                .await
            }
        }
    }

    pub async fn rename_file(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.rename_file(id, new_name).await,
            LbInner::Remote(r) => {
                r.call(Request::RenameFile { id: *id, new_name: new_name.to_string() }).await
            }
        }
    }

    pub async fn move_file(&self, id: &Uuid, new_parent: &Uuid) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.move_file(id, new_parent).await,
            LbInner::Remote(r) => {
                r.call(Request::MoveFile { id: *id, new_parent: *new_parent }).await
            }
        }
    }

    pub async fn delete(&self, id: &Uuid) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.delete(id).await,
            LbInner::Remote(r) => r.call(Request::Delete { id: *id }).await,
        }
    }

    pub async fn root(&self) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.root().await,
            LbInner::Remote(r) => r.call(Request::Root).await,
        }
    }

    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        match &self.inner {
            LbInner::Local(l) => l.list_metadatas().await,
            LbInner::Remote(r) => r.call(Request::ListMetadatas).await,
        }
    }

    pub async fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        match &self.inner {
            LbInner::Local(l) => l.get_children(id).await,
            LbInner::Remote(r) => r.call(Request::GetChildren { id: *id }).await,
        }
    }

    pub async fn get_and_get_children_recursively(
        &self, id: &Uuid,
    ) -> LbResult<Vec<File>> {
        match &self.inner {
            LbInner::Local(l) => l.get_and_get_children_recursively(id).await,
            LbInner::Remote(r) => {
                r.call(Request::GetAndGetChildrenRecursively { id: *id }).await
            }
        }
    }

    pub async fn get_file_by_id(&self, id: Uuid) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.get_file_by_id(id).await,
            LbInner::Remote(r) => r.call(Request::GetFileById { id }).await,
        }
    }

    pub async fn get_file_link_url(&self, id: Uuid) -> LbResult<String> {
        match &self.inner {
            LbInner::Local(l) => l.get_file_link_url(id).await,
            LbInner::Remote(r) => r.call(Request::GetFileLinkUrl { id }).await,
        }
    }

    pub async fn local_changes(&self) -> Vec<Uuid> {
        match &self.inner {
            LbInner::Local(l) => l.local_changes().await,
            LbInner::Remote(r) => r.call(Request::LocalChanges).await.unwrap_or_default(),
        }
    }

    // -- integrity --------------------------------------------------------

    pub async fn test_repo_integrity(&self, check_docs: bool) -> LbResult<Vec<Warning>> {
        match &self.inner {
            LbInner::Local(l) => l.test_repo_integrity(check_docs).await,
            LbInner::Remote(r) => r.call(Request::TestRepoIntegrity { check_docs }).await,
        }
    }

    // -- path -------------------------------------------------------------

    pub async fn create_link_at_path(
        &self, path: &str, target_id: Uuid,
    ) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.create_link_at_path(path, target_id).await,
            LbInner::Remote(r) => {
                r.call(Request::CreateLinkAtPath { path: path.to_string(), target_id })
                    .await
            }
        }
    }

    pub async fn create_at_path(&self, path: &str) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.create_at_path(path).await,
            LbInner::Remote(r) => r.call(Request::CreateAtPath { path: path.to_string() }).await,
        }
    }

    pub async fn get_by_path(&self, path: &str) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.get_by_path(path).await,
            LbInner::Remote(r) => r.call(Request::GetByPath { path: path.to_string() }).await,
        }
    }

    pub async fn get_path_by_id(&self, id: Uuid) -> LbResult<String> {
        match &self.inner {
            LbInner::Local(l) => l.get_path_by_id(id).await,
            LbInner::Remote(r) => r.call(Request::GetPathById { id }).await,
        }
    }

    pub async fn list_paths(&self, filter: Option<Filter>) -> LbResult<Vec<String>> {
        match &self.inner {
            LbInner::Local(l) => l.list_paths(filter).await,
            LbInner::Remote(r) => r.call(Request::ListPaths { filter }).await,
        }
    }

    pub async fn list_paths_with_ids(
        &self, filter: Option<Filter>,
    ) -> LbResult<Vec<(Uuid, String)>> {
        match &self.inner {
            LbInner::Local(l) => l.list_paths_with_ids(filter).await,
            LbInner::Remote(r) => r.call(Request::ListPathsWithIds { filter }).await,
        }
    }

    // -- share ------------------------------------------------------------

    pub async fn share_file(
        &self, id: Uuid, username: &str, mode: ShareMode,
    ) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.share_file(id, username, mode).await,
            LbInner::Remote(r) => {
                r.call(Request::ShareFile { id, username: username.to_string(), mode }).await
            }
        }
    }

    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>> {
        match &self.inner {
            LbInner::Local(l) => l.get_pending_shares().await,
            LbInner::Remote(r) => r.call(Request::GetPendingShares).await,
        }
    }

    pub async fn get_pending_share_files(&self) -> LbResult<Vec<File>> {
        match &self.inner {
            LbInner::Local(l) => l.get_pending_share_files().await,
            LbInner::Remote(r) => r.call(Request::GetPendingShareFiles).await,
        }
    }

    pub async fn known_usernames(&self) -> LbResult<Vec<String>> {
        match &self.inner {
            LbInner::Local(l) => l.known_usernames().await,
            LbInner::Remote(r) => r.call(Request::KnownUsernames).await,
        }
    }

    pub async fn reject_share(&self, id: &Uuid) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.reject_share(id).await,
            LbInner::Remote(r) => r.call(Request::RejectShare { id: *id }).await,
        }
    }

    // -- usage ------------------------------------------------------------

    pub async fn get_usage(&self) -> LbResult<UsageMetrics> {
        match &self.inner {
            LbInner::Local(l) => l.get_usage().await,
            LbInner::Remote(r) => r.call(Request::GetUsage).await,
        }
    }

    // -- subscribers ------------------------------------------------------

    pub async fn sync(&self) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.sync().await,
            LbInner::Remote(r) => r.call(Request::Sync).await,
        }
    }

    pub async fn status(&self) -> Status {
        match &self.inner {
            LbInner::Local(l) => l.status().await,
            LbInner::Remote(r) => r.call(Request::Status).await.unwrap_or_default(),
        }
    }

    pub async fn get_last_synced_human(&self) -> LbResult<String> {
        match &self.inner {
            LbInner::Local(l) => l.get_last_synced_human().await,
            LbInner::Remote(r) => r.call(Request::GetLastSyncedHuman).await,
        }
    }

    /// Subscribe to lb-rs events.
    ///
    /// Local: hands back a receiver from the in-process broadcast.
    /// Remote: hands back a receiver from the guest's relay broadcast,
    /// which the reader task populates from `Frame::Event` frames.
    /// `RemoteLb::connect` sends the host-side Subscribe eagerly, so by
    /// the time anyone calls this method the relay is already running.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<service::events::Event> {
        match &self.inner {
            LbInner::Local(l) => l.subscribe(),
            LbInner::Remote(r) => r.subscribe(),
        }
    }

    /// Pure formatting — no IPC. Identical impl to `LocalLb`.
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
    pub async fn search(
        &self, input: &str, cfg: SearchConfig,
    ) -> LbResult<Vec<SearchResult>> {
        match &self.inner {
            LbInner::Local(l) => l.search(input, cfg).await,
            LbInner::Remote(r) => r.call(Request::Search { input: input.to_string(), cfg }).await,
        }
    }
}

#[cfg(unix)]
async fn connect_guest_with_retry(
    socket: &std::path::Path,
    config: &Config,
) -> std::io::Result<ipc::client::RemoteLb> {
    let mut attempts: u32 = 0;
    let mut delay = std::time::Duration::from_millis(10);
    loop {
        match ipc::client::RemoteLb::connect(socket, config.clone()).await {
            Ok(c) => return Ok(c),
            Err(e) if attempts < 10 => {
                attempts += 1;
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, std::time::Duration::from_millis(500));
                let _ = e;
            }
            Err(e) => return Err(e),
        }
    }
}

// `Deref<Target = LocalLb>` is retained as a shim for the methods that
// don't yet have explicit forwarders — `get_account` and the
// `export_account_*` family. These are sync, return values, and need a
// Guest-side account cache to work without IPC. In Local mode the shim is
// transparent; in Remote mode it panics with a pointer to the deferred
// work.
impl std::ops::Deref for Lb {
    type Target = LocalLb;

    fn deref(&self) -> &LocalLb {
        match &self.inner {
            LbInner::Local(l) => l,
            #[cfg(unix)]
            LbInner::Remote(_) => panic!(
                "Lb::deref invoked in Remote (guest) mode; the called method is \
                 one of the deferred sync methods (get_account / \
                 export_account_*). These need a Guest-side account cache to \
                 land before they work over IPC."
            ),
        }
    }
}

impl std::ops::DerefMut for Lb {
    fn deref_mut(&mut self) -> &mut LocalLb {
        match &mut self.inner {
            LbInner::Local(l) => l,
            #[cfg(unix)]
            LbInner::Remote(_) => panic!(
                "Lb::deref_mut invoked in Remote (guest) mode; the called method \
                 is one of the deferred sync methods. See the Deref impl for \
                 details."
            ),
        }
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
use std::sync::Arc;
use subscribers::status::StatusUpdater;
use tokio::sync::RwLock;
pub use uuid::Uuid;
use web_time::Instant;

// Surface types referenced by the `Lb` forwarders. Hoisting them here
// keeps each forwarder body to its essential shape (a typed `Request`
// variant in, an `LbResult<Out>` out) instead of repeating fully-qualified
// paths.
use crate::ipc::protocol::Request;
use crate::model::account::{Account, Username};
use crate::model::api::{
    AccountFilter, AccountIdentifier, AccountInfo, AdminFileInfoResponse,
    AdminSetUserTierInfo, AdminValidateAccount, AdminValidateServer, ServerIndex,
    StripeAccountTier, SubscriptionInfo,
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
