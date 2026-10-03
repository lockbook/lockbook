use crate::Lb;
use crate::io::{CoreDb, CoreV5};
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file::File;
use crate::model::file_like::FileLike;
use crate::model::tree_like::TreeLike;
use crate::service::events::Actor;
use crate::service::keychain::Keychain;
use db_rs::View;
use db_rs::config::Config;
use db_rs::log::Log;
use std::path::Path;
use uuid::Uuid;

/// A one-shot local read for extensions: no runtime, sync, migration, or subscriptions.
pub fn read_pinned_documents(writeable_path: &Path) -> LbResult<Vec<File>> {
    let directory = writeable_path.join("CoreV5");
    let config = Config::default().log_location(&directory);
    if !directory.try_exists()? || Log::find_latest(&config)?.is_none() {
        return Err(LbErrKind::AccountNonexistent.into());
    }
    let db = CoreDb::init(&config)?;
    let db = &db.schema;
    let keychain = Keychain::from(db.account.as_ref());
    keychain.get_account()?;
    let ids = pinned_ids(db)?;
    let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
    tree.decrypt_all(&keychain, ids.into_iter(), &db.pub_key_lookup, false)
}

impl Lb {
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn pin_file(&self, id: Uuid) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let file = tree.maybe_find(&id).ok_or(LbErrKind::FileNonexistent)?;

        if !file.is_document() {
            return Err(LbErrKind::FileNotDocument.into());
        }

        if tree.calculate_deleted(&id)? {
            return Err(LbErrKind::FileNonexistent.into());
        }

        if db.pinned_files.as_slice().contains(&id) {
            return Ok(());
        }

        db.pinned_files.push(id)?;
        tx.end();
        self.events.meta_changed(Actor::User(None));
        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn unpin_file(&self, id: Uuid) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let entries: Vec<Uuid> = db
            .pinned_files
            .as_slice()
            .iter()
            .filter(|pinned| **pinned != id)
            .copied()
            .collect();
        if entries.len() == db.pinned_files.as_slice().len() {
            return Ok(());
        }

        db.pinned_files.clear()?;
        for entry in entries {
            db.pinned_files.push(entry)?;
        }
        tx.end();
        self.events.meta_changed(Actor::User(None));

        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_pinned(&self) -> LbResult<Vec<Uuid>> {
        let db = self.ro_tx().await;
        pinned_ids(db.db())
    }
}

fn pinned_ids(db: &CoreV5) -> LbResult<Vec<Uuid>> {
    let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
    let mut result = Vec::new();
    for id in db.pinned_files.as_slice() {
        if tree.maybe_find(id).is_none() {
            continue;
        }
        if tree.calculate_deleted(id)? || tree.in_pending_share(id)? {
            continue;
        }
        result.push(*id);
    }
    Ok(result)
}
