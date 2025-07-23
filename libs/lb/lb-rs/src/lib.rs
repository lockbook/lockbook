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
pub mod model;
pub mod service;
pub mod subscribers;

#[derive(Clone)]
pub struct Lb {
    pub config: Config,
    pub keychain: Keychain,
    pub db: LbDb,
    pub docs: AsyncDocs,
    pub search: SearchIndex,
    pub client: Network,
    pub events: EventSubs,
    pub syncing: Arc<AtomicBool>,
    pub status: StatusUpdater,
}

impl Lb {
    #[instrument(level = "info", skip_all, err(Debug))]
    pub async fn init(config: Config) -> LbResult<Self> {
        logging::init(&config)?;

        let db = CoreDb::init(db_rs::Config::in_folder(&config.writeable_path))
            .map_err(|err| LbErrKind::Unexpected(format!("{err:#?}")))?;
        let keychain = Keychain::from(db.account.get());
        let db = Arc::new(RwLock::new(db));
        let docs = AsyncDocs::from(&config);
        let client = Network::default();
        let search = SearchIndex::default();
        let status = StatusUpdater::default();
        let syncing = Arc::default();
        let events = EventSubs::default();

        let result = Self { config, keychain, db, docs, client, search, syncing, events, status };

        result.setup_search();
        result.setup_status().await?;

        Ok(result)
    }
}

pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub static DEFAULT_API_LOCATION: &str = "https://api.prod.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");

use crate::io::CoreDb;
use crate::service::logging;
use db_rs::Db;
use io::LbDb;
use io::docs::AsyncDocs;
use io::network::Network;
use model::core_config::Config;
pub use model::errors::{LbErrKind, LbResult};
use service::events::EventSubs;
use service::keychain::Keychain;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use subscribers::search::SearchIndex;
use subscribers::status::StatusUpdater;
use tokio::sync::RwLock;
pub use uuid::Uuid;
