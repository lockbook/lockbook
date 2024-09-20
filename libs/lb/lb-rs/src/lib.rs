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

pub mod logic;
pub mod model;
pub mod service;

// todo make this not pub 
pub mod repo;

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
    pub async fn init(config: Config) -> Result<Self, UnexpectedError> {
        logging::init(&config)?;

        let db = CoreDb::init(db_rs::Config::in_folder(&config.writeable_path))
            .map_err(|err| unexpected_only!("{:#?}", err))?;
        let db = Arc::new(RwLock::new(db));
        let docs = AsyncDocs::from(&config);
        let client = Network::default();
        let keychain = Keychain::default();
        let search = SearchIndex::default();
        Ok(Self { config, keychain, db, docs, client, search })
    }
}

pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub static DEFAULT_API_LOCATION: &str = "https://api.prod.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");

use model::core_config::Config;
use model::errors::UnexpectedError;
use repo::docs::AsyncDocs;
use repo::LbDb;
use service::keychain::Keychain;
use service::network::Network;
use service::search::SearchIndex;
use tokio::sync::RwLock;
use crate::repo::CoreDb;
use crate::service::logging;
use db_rs::Db;
use std::sync::Arc;
