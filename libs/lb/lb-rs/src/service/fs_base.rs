use crate::Lb;
use crate::io::FsBaseEntry;
use crate::model::errors::LbResult;
use uuid::Uuid;

impl Lb {
    /// Get all fs_base entries (the last-agreed state for sync-dir).
    pub async fn get_fs_base(&self) -> LbResult<Vec<(Uuid, FsBaseEntry)>> {
        let tx = self.ro_tx().await;
        let db = tx.db();
        Ok(db
            .fs_base
            .get()
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect())
    }

    /// Set fs_base entries in a single transaction. Clears existing entries first.
    pub async fn set_fs_base(&self, entries: Vec<(Uuid, FsBaseEntry)>) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.fs_base.clear().unwrap();
        for (id, entry) in entries {
            db.fs_base.insert(id, entry).unwrap();
        }
        tx.end();
        Ok(())
    }

    /// Clear all fs_base entries.
    pub async fn clear_fs_base(&self) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.fs_base.clear().unwrap();
        tx.end();
        Ok(())
    }
}
