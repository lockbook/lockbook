use db_rs::View;
use db_rs::config::Config as DbConfig;

use crate::Lb;
use crate::io::CoreDb;
use crate::io::docs::AsyncDocs;
use crate::io::network::Network;
use crate::model::core_config::Config;
use crate::model::errors::LbResult;

impl Lb {
    /// An in-memory instance for the browser. Builds through [Lb::assemble] like every other
    /// instance, so it gets each field a native one does; only the logging differs, since the page
    /// installs its own subscriber and has no files to write logs or panic reports to.
    pub fn init_dummy(config: Config) -> LbResult<Self> {
        if config.logs {
            crate::service::logging::log_startup(&config);
        }
        let db = CoreDb::init(&DbConfig::in_memory())?;
        let docs = AsyncDocs::from(&config);
        futures::executor::block_on(Self::assemble(config, db, docs, Network::default()))
    }
}
