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
//! See the [service] module for evolving this functionality.
//! - The [model] module contains the specification of our data structures and contracts between
//! components.
//! - The [logic] module contains our important algorithms and routines.
//!
//! - The `"blocking"` feature flag enables the [blocking] module and and the corresponding
//! blocking `Lb` variant.
//! - The `"ffi"` feature flag enables `blocking` as well as an API for C ffi clients
//! - The `"jni"` feature flag enables `blocking` as well as an API for JVM clients

#[macro_use]
extern crate tracing;

pub mod blocking;
pub mod logic;
pub mod model;
pub mod repo;
pub mod service;
pub mod text;

#[derive(Clone)]
pub struct Lb {
    pub config: Config,
    pub keychain: Keychain,
    pub db: LbDb,
    pub docs: AsyncDocs,
    pub search: SearchIndex,
    pub client: Network,
}

impl Lb {
    #[instrument(level = "info", skip_all, err(Debug))]
    pub async fn init(config: Config) -> LbResult<Self> {
        logging::init(&config)?;

        let db = CoreDb::init(db_rs::Config::in_folder(&config.writeable_path))
            .map_err(|err| LbErrKind::Unexpected(format!("{:#?}", err)))?;
        let keychain = Keychain::from(db.account.get());
        let db = Arc::new(RwLock::new(db));
        let docs = AsyncDocs::from(&config);
        let client = Network::default();
        let search = SearchIndex::default();

        let result = Self { config, keychain, db, docs, client, search };
        result.spawn_build_index();
        Ok(result)
    }
}

pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub static DEFAULT_API_LOCATION: &str = "https://api.prod.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");

use crate::repo::CoreDb;
use crate::service::logging;
use db_rs::Db;
use model::core_config::Config;
use model::errors::{LbErrKind, LbResult};
use repo::docs::AsyncDocs;
use repo::LbDb;
use service::keychain::Keychain;
use service::network::Network;
use service::search::SearchIndex;
use std::sync::Arc;
use tokio::sync::RwLock;
pub use uuid::Uuid;
