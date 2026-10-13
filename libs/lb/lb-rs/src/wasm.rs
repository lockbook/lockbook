use std::sync::Arc;

use db_rs::View;
use db_rs::config::Config as DbConfig;
use tokio::sync::RwLock;
use web_time::Instant;

use crate::Lb;
use crate::io::CoreDb;
use crate::io::docs::AsyncDocs;
use crate::model::core_config::Config;
use crate::model::errors::LbResult;

impl Lb {
    pub fn init_dummy(config: Config) -> LbResult<Self> {
        let db = CoreDb::init(&DbConfig::in_memory())?;
        let user_last_seen = Arc::new(RwLock::new(Instant::now()));

        Ok(Self {
            user_last_seen,
            user_wake: Default::default(),
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
