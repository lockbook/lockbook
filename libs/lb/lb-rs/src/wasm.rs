use std::sync::{Arc, OnceLock};

use db_rs::Db;
use tokio::sync::RwLock;
use web_time::Instant;

use crate::io::CoreDb;
use crate::io::docs::AsyncDocs;
use crate::model::core_config::Config;
use crate::model::errors::{LbErrKind, LbResult};
use crate::{Lb, LocalLb};

impl LocalLb {
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

impl Lb {
    pub fn init_dummy(config: Config) -> LbResult<Self> {
        let loc = LocalLb::init_dummy(config.clone())?;
        let local = Arc::new(OnceLock::new());
        let _ = local.set(loc);
        Ok(Self { local, remote: None, config })
    }
}
