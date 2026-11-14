//! Independent copies of in-memory instances, for tests that build a world once and start many
//! cases from it.
use crate::io::docs::LocalDocuments;

use db_rs::View;
use db_rs::config::Config as DbConfig;
use db_rs::errors::Result as DbResult;
use db_rs::views::composite_view::{Composite, Schema};

use crate::Lb;
use crate::io::CoreDb;
use crate::io::server::Server;
use crate::model::core_config::Config;
use crate::model::errors::{LbErrKind, LbResult};
use crate::service::logging;

/// An in-memory database holding what `db` holds, sharing nothing with it. Every table restarts
/// at sequence number zero, as the copy's log does.
pub fn copy_db<S: Schema>(db: &mut Composite<S>) -> DbResult<Composite<S>> {
    let mut copy = Composite::<S>::init(&DbConfig::in_memory())?;
    let mut tables = db.schema.views_mut();
    let mut copies = copy.schema.views_mut();
    for (table, copy) in tables.as_mut().iter_mut().zip(copies.as_mut().iter_mut()) {
        copy.handle_events(0, &table.generate_snapshot()?)?;
    }
    drop(copies);
    Ok(copy)
}

impl<S: Server, D: LocalDocuments> Lb<S, D> {
    /// An in-memory instance holding this one's database and the documents `docs` copies from
    /// this one's, sharing nothing with it, configured by `config` and sending every request to
    /// `server`. It starts as this instance would after a restart: nothing in flight, caches
    /// empty. Waits for any sync in progress.
    pub async fn copy_in_memory<T: Server, E: LocalDocuments>(
        &self, config: Config, server: T, docs: impl FnOnce(&D) -> E,
    ) -> LbResult<Lb<T, E>> {
        let _syncing = self.syncer.lock().await;
        let mut db = self.db.write().await;
        let db: CoreDb =
            copy_db(&mut db).map_err(|err| LbErrKind::Unexpected(format!("{err:?}")))?;
        let docs = docs(&self.docs);

        logging::init(&config)?;
        Lb::assemble(
            config,
            db,
            docs,
            server,
            #[cfg(all(unix, not(target_os = "ios")))]
            None,
            #[cfg(not(any(target_family = "wasm", target_os = "ios")))]
            None,
        )
        .await
    }
}
