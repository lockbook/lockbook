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

#[derive(Clone)]
pub struct Lb {
    pub config: Config,
    pub user_last_seen: Arc<RwLock<Instant>>,
    pub user_wake: Arc<Notify>,
    pub keychain: Keychain,
    pub db: LbDb,
    pub docs: AsyncDocs,
    pub client: Network,
    pub events: EventSubs,
    pub status: StatusUpdater,
    pub syncer: Syncer,
}

impl Lb {
    #[instrument(name = "Lb::init", level = "info", skip_all, err(Debug))]
    pub async fn init(config: Config) -> LbResult<Self> {
        logging::init(&config)?;

        let docs = AsyncDocs::from(&config);
        let db = io::init_with_migration(Path::new(&config.writeable_path))
            .map_err(|err| LbErrKind::Unexpected(format!("{err:#?}")))?;
        let keychain = Keychain::from(db.schema.account.as_ref());
        let db = Arc::new(RwLock::new(db));
        let client = Network { client_type: config.client_type, ..Network::default() };

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
        };

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

pub static DEFAULT_API_LOCATION: &str = "https://app.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");

use crate::subscribers::syncer::Syncer;

use crate::service::logging;
use io::LbDb;
use io::docs::AsyncDocs;
use io::network::Network;
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
