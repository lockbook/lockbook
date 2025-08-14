use crate::io::docs::AsyncDocs;
use crate::io::network::Network;
use crate::io::CoreDb;
use crate::io::LbDb;
use crate::model::core_config::Config;
use crate::model::errors::{LbErrKind, LbResult};
use crate::service::events::EventSubs;
use crate::service::keychain::Keychain;
use crate::service::logging;
use crate::subscribers::search::SearchIndex;
use crate::subscribers::status::StatusUpdater;
use db_rs::Db;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct LbServer {
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

impl LbServer {
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
        let status = StatusUpdater::default();
        let syncing = Arc::default();
        let events = EventSubs::default();

        let result = Self { config, keychain, db, docs, client, search, syncing, events, status };

        result.setup_search();
        result.setup_status().await?;

        Ok(result)
    }
}
