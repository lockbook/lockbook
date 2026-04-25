use crate::LocalLb;
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file_like::FileLike;
use crate::model::tree_like::TreeLike;
use uuid::Uuid;

impl LocalLb {
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

        if db.pinned_files.get().contains(&id) {
            return Ok(());
        }

        db.pinned_files.push(id)?;
        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn unpin_file(&self, id: Uuid) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let entries: Vec<Uuid> = db
            .pinned_files
            .get()
            .iter()
            .filter(|pinned| **pinned != id)
            .copied()
            .collect();

        db.pinned_files.clear()?;
        for entry in entries {
            db.pinned_files.push(entry)?;
        }

        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_pinned(&self) -> LbResult<Vec<Uuid>> {
        let db = self.ro_tx().await;
        let db = db.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let mut result = Vec::new();
        for id in db.pinned_files.get().iter() {
            if tree.maybe_find(id).is_none() {
                continue;
            }
            if tree.calculate_deleted(id)? {
                continue;
            }
            if tree.in_pending_share(id)? {
                continue;
            }
            result.push(*id);
        }

        Ok(result)
    }
}
