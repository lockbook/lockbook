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
pub mod experiments;
pub mod io;
pub mod macros;
pub mod model;
pub mod search;
pub mod service;
pub mod subscribers;
#[cfg(target_family = "wasm")]
pub mod wasm;

// C headers name only `blocking::Lb`; cbindgen keys types by bare name, so this one stays out.
/// cbindgen:ignore
#[derive(Clone)]
pub struct Lb<S = Network> {
    pub config: Config,
    pub user_last_seen: Arc<RwLock<Instant>>,
    pub user_wake: Arc<Notify>,
    pub keychain: Keychain,
    pub db: LbDb,
    pub docs: AsyncDocs,
    pub client: S,
    pub events: EventSubs,
    pub status: StatusUpdater,
    pub syncer: Syncer,
    #[cfg(all(unix, not(target_os = "ios")))]
    /// None for an instance that shares its database with no other process.
    pub(crate) ipc: Option<Arc<Ipc>>,
    #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
    /// The file that keeps two instances on one data directory from syncing at once; None for an
    /// instance with no data directory.
    pub(crate) sync_lock: Option<std::path::PathBuf>,
}

impl Lb {
    #[instrument(name = "Lb::init", level = "info", skip_all, err(Debug))]
    pub async fn init(config: Config) -> LbResult<Self> {
        logging::init(&config)?;

        let docs = AsyncDocs::from(&config);
        let db = io::init_with_migration(Path::new(&config.writeable_path))
            .map_err(|err| LbErrKind::Unexpected(format!("{err:#?}")))?;
        #[cfg(all(unix, not(target_os = "ios")))]
        let ipc = Some(Arc::new(Ipc::new(&db, config.background_work)?));
        #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
        let sync_lock = Some(Path::new(&config.writeable_path).join("sync.lock"));
        let client = Network { client_type: config.client_type, ..Network::default() };

        Self::assemble(
            config,
            db,
            docs,
            client,
            #[cfg(all(unix, not(target_os = "ios")))]
            ipc,
            #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
            sync_lock,
        )
        .await
    }
}

impl<S: Server> Lb<S> {
    /// An instance whose database and documents live in memory, shared with no other process,
    /// that sends every request to `server`. Only logs and panic reports use `config.writeable_path`.
    pub async fn in_memory(config: Config, server: S) -> LbResult<Self> {
        logging::init(&config)?;

        let db = io::CoreDb::init(&db_rs::config::Config::in_memory())?;

        Self::assemble(
            config,
            db,
            AsyncDocs::in_memory(),
            server,
            #[cfg(all(unix, not(target_os = "ios")))]
            None,
            #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
            None,
        )
        .await
    }

    async fn assemble(
        config: Config, db: io::CoreDb, docs: AsyncDocs, client: S,
        #[cfg(all(unix, not(target_os = "ios")))] ipc: Option<Arc<Ipc>>,
        #[cfg(not(any(target_family = "wasm", target_os = "ios")))] sync_lock: Option<
            std::path::PathBuf,
        >,
    ) -> LbResult<Self> {
        let keychain = Keychain::from(db.schema.account.as_ref());
        let db = Arc::new(RwLock::new(db));

        let status = StatusUpdater::default();
        let syncer = Default::default();
        let events = EventSubs::default();
        let user_last_seen = Arc::new(RwLock::new(Instant::now()));
        let user_wake = Arc::new(Notify::new());

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
            user_wake,
            #[cfg(all(unix, not(target_os = "ios")))]
            ipc,
            #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
            sync_lock,
        };

        #[cfg(all(unix, not(target_os = "ios")))]
        result.setup_ipc().await?;

        #[cfg(not(target_family = "wasm"))]
        {
            result.setup_syncer();
            result.setup_status().await?;
        }

        Ok(result)
    }
}

pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn get_code_commit() -> Option<&'static str> {
    Some(env!("LB_GIT_COMMIT")).filter(|commit| *commit != "unknown")
}

pub static DEFAULT_API_LOCATION: &str = "https://app.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(all(unix, not(target_os = "ios")))]
use crate::subscribers::ipc::Ipc;
use crate::subscribers::syncer::Syncer;

use crate::service::logging;
use db_rs::View;
use io::LbDb;
use io::docs::AsyncDocs;
use io::network::Network;
use io::server::Server;
use model::core_config::Config;
pub use model::errors::{LbErrKind, LbResult};
use service::events::EventSubs;
use service::keychain::Keychain;
use std::path::Path;
use std::sync::Arc;
use subscribers::status::StatusUpdater;
use tokio::sync::{Notify, RwLock};
pub use uuid::Uuid;
use web_time::Instant;
