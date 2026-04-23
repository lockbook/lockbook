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
/// Wraps an `LbInner` whose only variant today is `Local(LocalLb)` — the in-process
/// implementation. A future `Remote` variant will forward calls over IPC to a host
/// process when this client cannot acquire the db-rs filesystem lock.
///
/// Stage 1 leans on `Deref<Target = LocalLb>` so consumers see the same surface the
/// pre-wrapper `Lb` had. When `Remote` lands, `Deref` is removed and every public
/// method on `Lb` becomes an explicit forwarder that dispatches on `inner`.
#[derive(Clone)]
pub struct Lb {
    inner: LbInner,
}

#[derive(Clone)]
enum LbInner {
    Local(LocalLb),
}

impl Lb {
    pub async fn init(config: Config) -> LbResult<Self> {
        let local = LocalLb::init(config).await?;
        Ok(Lb { inner: LbInner::Local(local) })
    }

    /// see [`LocalLb::init_dummy`]
    #[cfg(target_family = "wasm")]
    pub fn init_dummy(config: Config) -> LbResult<Self> {
        let local = LocalLb::init_dummy(config)?;
        Ok(Lb { inner: LbInner::Local(local) })
    }
}

impl std::ops::Deref for Lb {
    type Target = LocalLb;

    fn deref(&self) -> &LocalLb {
        match &self.inner {
            LbInner::Local(l) => l,
        }
    }
}

impl std::ops::DerefMut for Lb {
    fn deref_mut(&mut self) -> &mut LocalLb {
        match &mut self.inner {
            LbInner::Local(l) => l,
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
