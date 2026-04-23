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
#[cfg(not(target_family = "wasm"))]
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
    #[cfg(unix)]
    Remote(ipc::client::RemoteLb),
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
// One forwarder per ported `LocalLb` method: dispatches on `inner` to
// either run locally or send over IPC. Adding a new method means adding a
// `Request`/`Response` variant pair, a `match` arm in
// `ipc::server::dispatch`, and a forwarder here.
//
// The following methods are *not* forwarded explicitly and remain
// reachable only through the `Deref` shim (Local-only; Remote panics):
//
//   - `get_account`, `export_account_private_key`, `export_account_phrase`,
//     `export_account_qr` — sync, return values that need the in-memory
//     account. A future pass should cache the account on the Guest at
//     connect time so these can stay sync without IPC.
//   - `subscribe` — long-lived event stream, deferred along with the
//     subscriber API redesign described in `ipc/mod.rs`.

impl Lb {
    // -- account ----------------------------------------------------------

    pub async fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        match &self.inner {
            LbInner::Local(l) => l.create_account(username, api_url, welcome_doc).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::CreateAccount {
                    username: username.to_string(),
                    api_url: api_url.to_string(),
                    welcome_doc,
                })
                .await?
            {
                Response::CreateAccount(res) => res,
                _ => Err(ipc_response_mismatch("CreateAccount")),
            },
        }
    }

    pub async fn import_account(
        &self, key: &str, api_url: Option<&str>,
    ) -> LbResult<Account> {
        match &self.inner {
            LbInner::Local(l) => l.import_account(key, api_url).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::ImportAccount {
                    key: key.to_string(),
                    api_url: api_url.map(|s| s.to_string()),
                })
                .await?
            {
                Response::ImportAccount(res) => res,
                _ => Err(ipc_response_mismatch("ImportAccount")),
            },
        }
    }

    pub async fn import_account_private_key_v1(
        &self, account: Account,
    ) -> LbResult<Account> {
        match &self.inner {
            LbInner::Local(l) => l.import_account_private_key_v1(account).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::ImportAccountPrivateKeyV1 { account })
                .await?
            {
                Response::ImportAccountPrivateKeyV1(res) => res,
                _ => Err(ipc_response_mismatch("ImportAccountPrivateKeyV1")),
            },
        }
    }

    pub async fn import_account_phrase(
        &self, phrase: [&str; 24], api_url: &str,
    ) -> LbResult<Account> {
        match &self.inner {
            LbInner::Local(l) => l.import_account_phrase(phrase, api_url).await,
            #[cfg(unix)]
            LbInner::Remote(r) => {
                let phrase: [String; 24] = std::array::from_fn(|i| phrase[i].to_string());
                match r
                    .call(Request::ImportAccountPhrase {
                        phrase,
                        api_url: api_url.to_string(),
                    })
                    .await?
                {
                    Response::ImportAccountPhrase(res) => res,
                    _ => Err(ipc_response_mismatch("ImportAccountPhrase")),
                }
            }
        }
    }

    pub async fn delete_account(&self) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.delete_account().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::DeleteAccount).await? {
                Response::DeleteAccount(res) => res,
                _ => Err(ipc_response_mismatch("DeleteAccount")),
            },
        }
    }

    // -- activity ---------------------------------------------------------

    pub async fn suggested_docs(
        &self, settings: RankingWeights,
    ) -> LbResult<Vec<Uuid>> {
        match &self.inner {
            LbInner::Local(l) => l.suggested_docs(settings).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::SuggestedDocs { settings })
                .await?
            {
                Response::SuggestedDocs(res) => res,
                _ => Err(ipc_response_mismatch("SuggestedDocs")),
            },
        }
    }

    pub async fn clear_suggested(&self) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.clear_suggested().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::ClearSuggested).await? {
                Response::ClearSuggested(res) => res,
                _ => Err(ipc_response_mismatch("ClearSuggested")),
            },
        }
    }

    pub async fn clear_suggested_id(&self, id: Uuid) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.clear_suggested_id(id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::ClearSuggestedId { id })
                .await?
            {
                Response::ClearSuggestedId(res) => res,
                _ => Err(ipc_response_mismatch("ClearSuggestedId")),
            },
        }
    }

    /// Hint the host that the user is around. Sync — fire-and-forget for
    /// guests so the existing sync caller surface doesn't change.
    pub fn app_foregrounded(&self) {
        match &self.inner {
            LbInner::Local(l) => l.app_foregrounded(),
            #[cfg(unix)]
            LbInner::Remote(r) => {
                let r = r.clone();
                tokio::spawn(async move {
                    let _ = r.call(Request::AppForegrounded).await;
                });
            }
        }
    }

    // -- admin ------------------------------------------------------------

    pub async fn disappear_account(&self, username: &str) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.disappear_account(username).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::DisappearAccount { username: username.to_string() })
                .await?
            {
                Response::DisappearAccount(res) => res,
                _ => Err(ipc_response_mismatch("DisappearAccount")),
            },
        }
    }

    pub async fn disappear_file(&self, id: Uuid) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.disappear_file(id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::DisappearFile { id })
                .await?
            {
                Response::DisappearFile(res) => res,
                _ => Err(ipc_response_mismatch("DisappearFile")),
            },
        }
    }

    pub async fn list_users(
        &self, filter: Option<AccountFilter>,
    ) -> LbResult<Vec<Username>> {
        match &self.inner {
            LbInner::Local(l) => l.list_users(filter).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::ListUsers { filter })
                .await?
            {
                Response::ListUsers(res) => res,
                _ => Err(ipc_response_mismatch("ListUsers")),
            },
        }
    }

    pub async fn get_account_info(
        &self, identifier: AccountIdentifier,
    ) -> LbResult<AccountInfo> {
        match &self.inner {
            LbInner::Local(l) => l.get_account_info(identifier).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::GetAccountInfo { identifier })
                .await?
            {
                Response::GetAccountInfo(res) => res,
                _ => Err(ipc_response_mismatch("GetAccountInfo")),
            },
        }
    }

    pub async fn validate_account(
        &self, username: &str,
    ) -> LbResult<AdminValidateAccount> {
        match &self.inner {
            LbInner::Local(l) => l.validate_account(username).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::AdminValidateAccount {
                    username: username.to_string(),
                })
                .await?
            {
                Response::AdminValidateAccount(res) => res,
                _ => Err(ipc_response_mismatch("AdminValidateAccount")),
            },
        }
    }

    pub async fn validate_server(&self) -> LbResult<AdminValidateServer> {
        match &self.inner {
            LbInner::Local(l) => l.validate_server().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::AdminValidateServer)
                .await?
            {
                Response::AdminValidateServer(res) => res,
                _ => Err(ipc_response_mismatch("AdminValidateServer")),
            },
        }
    }

    pub async fn file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse> {
        match &self.inner {
            LbInner::Local(l) => l.file_info(id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::AdminFileInfo { id })
                .await?
            {
                Response::AdminFileInfo(res) => res,
                _ => Err(ipc_response_mismatch("AdminFileInfo")),
            },
        }
    }

    pub async fn rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.rebuild_index(index).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::RebuildIndex { index })
                .await?
            {
                Response::RebuildIndex(res) => res,
                _ => Err(ipc_response_mismatch("RebuildIndex")),
            },
        }
    }

    pub async fn set_user_tier(
        &self, username: &str, info: AdminSetUserTierInfo,
    ) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.set_user_tier(username, info).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::SetUserTier {
                    username: username.to_string(),
                    info,
                })
                .await?
            {
                Response::SetUserTier(res) => res,
                _ => Err(ipc_response_mismatch("SetUserTier")),
            },
        }
    }

    // -- billing ----------------------------------------------------------

    pub async fn upgrade_account_stripe(
        &self, account_tier: StripeAccountTier,
    ) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.upgrade_account_stripe(account_tier).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::UpgradeAccountStripe { account_tier })
                .await?
            {
                Response::UpgradeAccountStripe(res) => res,
                _ => Err(ipc_response_mismatch("UpgradeAccountStripe")),
            },
        }
    }

    pub async fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.upgrade_account_google_play(purchase_token, account_id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::UpgradeAccountGooglePlay {
                    purchase_token: purchase_token.to_string(),
                    account_id: account_id.to_string(),
                })
                .await?
            {
                Response::UpgradeAccountGooglePlay(res) => res,
                _ => Err(ipc_response_mismatch("UpgradeAccountGooglePlay")),
            },
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
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::UpgradeAccountAppStore {
                    original_transaction_id,
                    app_account_token,
                })
                .await?
            {
                Response::UpgradeAccountAppStore(res) => res,
                _ => Err(ipc_response_mismatch("UpgradeAccountAppStore")),
            },
        }
    }

    pub async fn cancel_subscription(&self) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.cancel_subscription().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::CancelSubscription)
                .await?
            {
                Response::CancelSubscription(res) => res,
                _ => Err(ipc_response_mismatch("CancelSubscription")),
            },
        }
    }

    pub async fn get_subscription_info(
        &self,
    ) -> LbResult<Option<SubscriptionInfo>> {
        match &self.inner {
            LbInner::Local(l) => l.get_subscription_info().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::GetSubscriptionInfo)
                .await?
            {
                Response::GetSubscriptionInfo(res) => res,
                _ => Err(ipc_response_mismatch("GetSubscriptionInfo")),
            },
        }
    }

    // -- debug (cfg!=wasm) -----------------------------------------------

    #[cfg(not(target_family = "wasm"))]
    pub async fn recent_panic(&self) -> LbResult<bool> {
        match &self.inner {
            LbInner::Local(l) => l.recent_panic().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::RecentPanic).await? {
                Response::RecentPanic(res) => res,
                _ => Err(ipc_response_mismatch("RecentPanic")),
            },
        }
    }

    #[cfg(not(target_family = "wasm"))]
    pub async fn write_panic_to_file(
        &self, error_header: String, bt: String,
    ) -> LbResult<String> {
        match &self.inner {
            LbInner::Local(l) => l.write_panic_to_file(error_header, bt).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::WritePanicToFile { error_header, bt })
                .await?
            {
                Response::WritePanicToFile(res) => res,
                _ => Err(ipc_response_mismatch("WritePanicToFile")),
            },
        }
    }

    #[cfg(not(target_family = "wasm"))]
    pub async fn debug_info(
        &self, os_info: String, check_docs: bool,
    ) -> LbResult<DebugInfo> {
        match &self.inner {
            LbInner::Local(l) => l.debug_info(os_info, check_docs).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::DebugInfo { os_info, check_docs })
                .await?
            {
                Response::DebugInfo(res) => res,
                _ => Err(ipc_response_mismatch("DebugInfo")),
            },
        }
    }

    // -- documents --------------------------------------------------------

    pub async fn read_document(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<DecryptedDocument> {
        match &self.inner {
            LbInner::Local(l) => l.read_document(id, user_activity).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::ReadDocument { id, user_activity })
                .await?
            {
                Response::ReadDocument(res) => res,
                _ => Err(ipc_response_mismatch("ReadDocument")),
            },
        }
    }

    pub async fn write_document(&self, id: Uuid, content: &[u8]) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.write_document(id, content).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::WriteDocument { id, content: content.to_vec() })
                .await?
            {
                Response::WriteDocument(res) => res,
                _ => Err(ipc_response_mismatch("WriteDocument")),
            },
        }
    }

    pub async fn read_document_with_hmac(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)>
    {
        match &self.inner {
            LbInner::Local(l) => l.read_document_with_hmac(id, user_activity).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::ReadDocumentWithHmac { id, user_activity })
                .await?
            {
                Response::ReadDocumentWithHmac(res) => res,
                _ => Err(ipc_response_mismatch("ReadDocumentWithHmac")),
            },
        }
    }

    pub async fn safe_write(
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        match &self.inner {
            LbInner::Local(l) => l.safe_write(id, old_hmac, content).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::SafeWrite { id, old_hmac, content })
                .await?
            {
                Response::SafeWrite(res) => res,
                _ => Err(ipc_response_mismatch("SafeWrite")),
            },
        }
    }

    // -- file -------------------------------------------------------------

    pub async fn create_file(
        &self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.create_file(name, parent, file_type).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::CreateFile {
                    name: name.to_string(),
                    parent: *parent,
                    file_type,
                })
                .await?
            {
                Response::CreateFile(res) => res,
                _ => Err(ipc_response_mismatch("CreateFile")),
            },
        }
    }

    pub async fn rename_file(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.rename_file(id, new_name).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::RenameFile {
                    id: *id,
                    new_name: new_name.to_string(),
                })
                .await?
            {
                Response::RenameFile(res) => res,
                _ => Err(ipc_response_mismatch("RenameFile")),
            },
        }
    }

    pub async fn move_file(&self, id: &Uuid, new_parent: &Uuid) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.move_file(id, new_parent).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::MoveFile { id: *id, new_parent: *new_parent })
                .await?
            {
                Response::MoveFile(res) => res,
                _ => Err(ipc_response_mismatch("MoveFile")),
            },
        }
    }

    pub async fn delete(&self, id: &Uuid) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.delete(id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::Delete { id: *id })
                .await?
            {
                Response::Delete(res) => res,
                _ => Err(ipc_response_mismatch("Delete")),
            },
        }
    }

    pub async fn root(&self) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.root().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::Root).await? {
                Response::Root(res) => res,
                _ => Err(ipc_response_mismatch("Root")),
            },
        }
    }

    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        match &self.inner {
            LbInner::Local(l) => l.list_metadatas().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::ListMetadatas).await? {
                Response::ListMetadatas(res) => res,
                _ => Err(ipc_response_mismatch("ListMetadatas")),
            },
        }
    }

    pub async fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        match &self.inner {
            LbInner::Local(l) => l.get_children(id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::GetChildren { id: *id })
                .await?
            {
                Response::GetChildren(res) => res,
                _ => Err(ipc_response_mismatch("GetChildren")),
            },
        }
    }

    pub async fn get_and_get_children_recursively(
        &self, id: &Uuid,
    ) -> LbResult<Vec<File>> {
        match &self.inner {
            LbInner::Local(l) => l.get_and_get_children_recursively(id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::GetAndGetChildrenRecursively { id: *id })
                .await?
            {
                Response::GetAndGetChildrenRecursively(res) => res,
                _ => Err(ipc_response_mismatch("GetAndGetChildrenRecursively")),
            },
        }
    }

    pub async fn get_file_by_id(&self, id: Uuid) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.get_file_by_id(id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::GetFileById { id })
                .await?
            {
                Response::GetFileById(res) => res,
                _ => Err(ipc_response_mismatch("GetFileById")),
            },
        }
    }

    pub async fn get_file_link_url(&self, id: Uuid) -> LbResult<String> {
        match &self.inner {
            LbInner::Local(l) => l.get_file_link_url(id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::GetFileLinkUrl { id })
                .await?
            {
                Response::GetFileLinkUrl(res) => res,
                _ => Err(ipc_response_mismatch("GetFileLinkUrl")),
            },
        }
    }

    pub async fn local_changes(&self) -> Vec<Uuid> {
        match &self.inner {
            LbInner::Local(l) => l.local_changes().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::LocalChanges).await {
                Ok(Response::LocalChanges(v)) => v,
                _ => {
                    tracing::warn!("ipc LocalChanges call failed; returning empty");
                    Vec::new()
                }
            },
        }
    }

    // -- integrity --------------------------------------------------------

    pub async fn test_repo_integrity(
        &self, check_docs: bool,
    ) -> LbResult<Vec<Warning>> {
        match &self.inner {
            LbInner::Local(l) => l.test_repo_integrity(check_docs).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::TestRepoIntegrity { check_docs })
                .await?
            {
                Response::TestRepoIntegrity(res) => res,
                _ => Err(ipc_response_mismatch("TestRepoIntegrity")),
            },
        }
    }

    // -- path -------------------------------------------------------------

    pub async fn create_link_at_path(
        &self, path: &str, target_id: Uuid,
    ) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.create_link_at_path(path, target_id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::CreateLinkAtPath {
                    path: path.to_string(),
                    target_id,
                })
                .await?
            {
                Response::CreateLinkAtPath(res) => res,
                _ => Err(ipc_response_mismatch("CreateLinkAtPath")),
            },
        }
    }

    pub async fn create_at_path(&self, path: &str) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.create_at_path(path).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::CreateAtPath { path: path.to_string() })
                .await?
            {
                Response::CreateAtPath(res) => res,
                _ => Err(ipc_response_mismatch("CreateAtPath")),
            },
        }
    }

    pub async fn get_by_path(&self, path: &str) -> LbResult<File> {
        match &self.inner {
            LbInner::Local(l) => l.get_by_path(path).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::GetByPath { path: path.to_string() })
                .await?
            {
                Response::GetByPath(res) => res,
                _ => Err(ipc_response_mismatch("GetByPath")),
            },
        }
    }

    pub async fn get_path_by_id(&self, id: Uuid) -> LbResult<String> {
        match &self.inner {
            LbInner::Local(l) => l.get_path_by_id(id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::GetPathById { id })
                .await?
            {
                Response::GetPathById(res) => res,
                _ => Err(ipc_response_mismatch("GetPathById")),
            },
        }
    }

    pub async fn list_paths(
        &self, filter: Option<Filter>,
    ) -> LbResult<Vec<String>> {
        match &self.inner {
            LbInner::Local(l) => l.list_paths(filter).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::ListPaths { filter })
                .await?
            {
                Response::ListPaths(res) => res,
                _ => Err(ipc_response_mismatch("ListPaths")),
            },
        }
    }

    pub async fn list_paths_with_ids(
        &self, filter: Option<Filter>,
    ) -> LbResult<Vec<(Uuid, String)>> {
        match &self.inner {
            LbInner::Local(l) => l.list_paths_with_ids(filter).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::ListPathsWithIds { filter })
                .await?
            {
                Response::ListPathsWithIds(res) => res,
                _ => Err(ipc_response_mismatch("ListPathsWithIds")),
            },
        }
    }

    // -- share ------------------------------------------------------------

    pub async fn share_file(
        &self, id: Uuid, username: &str, mode: ShareMode,
    ) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.share_file(id, username, mode).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::ShareFile {
                    id,
                    username: username.to_string(),
                    mode,
                })
                .await?
            {
                Response::ShareFile(res) => res,
                _ => Err(ipc_response_mismatch("ShareFile")),
            },
        }
    }

    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>> {
        match &self.inner {
            LbInner::Local(l) => l.get_pending_shares().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::GetPendingShares).await? {
                Response::GetPendingShares(res) => res,
                _ => Err(ipc_response_mismatch("GetPendingShares")),
            },
        }
    }

    pub async fn get_pending_share_files(&self) -> LbResult<Vec<File>> {
        match &self.inner {
            LbInner::Local(l) => l.get_pending_share_files().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::GetPendingShareFiles)
                .await?
            {
                Response::GetPendingShareFiles(res) => res,
                _ => Err(ipc_response_mismatch("GetPendingShareFiles")),
            },
        }
    }

    pub async fn known_usernames(&self) -> LbResult<Vec<String>> {
        match &self.inner {
            LbInner::Local(l) => l.known_usernames().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::KnownUsernames).await? {
                Response::KnownUsernames(res) => res,
                _ => Err(ipc_response_mismatch("KnownUsernames")),
            },
        }
    }

    pub async fn reject_share(&self, id: &Uuid) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.reject_share(id).await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::RejectShare { id: *id })
                .await?
            {
                Response::RejectShare(res) => res,
                _ => Err(ipc_response_mismatch("RejectShare")),
            },
        }
    }

    // -- usage ------------------------------------------------------------

    pub async fn get_usage(&self) -> LbResult<UsageMetrics> {
        match &self.inner {
            LbInner::Local(l) => l.get_usage().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::GetUsage).await? {
                Response::GetUsage(res) => res,
                _ => Err(ipc_response_mismatch("GetUsage")),
            },
        }
    }

    // -- subscribers ------------------------------------------------------

    pub async fn sync(&self) -> LbResult<()> {
        match &self.inner {
            LbInner::Local(l) => l.sync().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::Sync).await? {
                Response::Sync(res) => res,
                _ => Err(ipc_response_mismatch("Sync")),
            },
        }
    }

    pub async fn status(&self) -> Status {
        match &self.inner {
            LbInner::Local(l) => l.status().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r.call(Request::Status).await {
                Ok(Response::Status(s)) => s,
                _ => {
                    tracing::warn!("ipc Status call failed; returning default");
                    Status::default()
                }
            },
        }
    }

    pub async fn get_last_synced_human(&self) -> LbResult<String> {
        match &self.inner {
            LbInner::Local(l) => l.get_last_synced_human().await,
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::GetLastSyncedHuman)
                .await?
            {
                Response::GetLastSyncedHuman(res) => res,
                _ => Err(ipc_response_mismatch("GetLastSyncedHuman")),
            },
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
            #[cfg(unix)]
            LbInner::Remote(r) => match r
                .call(Request::Search { input: input.to_string(), cfg })
                .await?
            {
                Response::Search(res) => res,
                _ => Err(ipc_response_mismatch("Search")),
            },
        }
    }
}

fn ipc_response_mismatch(expected: &'static str) -> model::errors::LbErr {
    LbErrKind::Unexpected(format!("ipc: expected Response::{expected}, got different variant"))
        .into()
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
// don't yet have explicit forwarders — `get_account`, `export_account_*`,
// and `subscribe`. These are sync, return values, and need either a
// Guest-side cache (the export/get-account family) or a separate streaming
// design (`subscribe`). In Local mode the shim is transparent; in Remote
// mode it panics with a pointer to the deferred work.
impl std::ops::Deref for Lb {
    type Target = LocalLb;

    fn deref(&self) -> &LocalLb {
        match &self.inner {
            LbInner::Local(l) => l,
            #[cfg(unix)]
            LbInner::Remote(_) => panic!(
                "Lb::deref invoked in Remote (guest) mode; the called method is \
                 one of the deferred sync methods (get_account / \
                 export_account_* / subscribe). These need a Guest-side account \
                 cache or the subscriber API to land before they work over IPC."
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
// keeps each forwarder body to its essential shape (one Request variant in,
// one Response variant out) instead of repeating fully-qualified paths.
#[cfg(unix)]
use crate::ipc::protocol::{Request, Response};
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
