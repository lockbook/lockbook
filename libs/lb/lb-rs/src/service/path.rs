use crate::Lb;
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file::File;
use crate::model::path_ops::Filter;
use crate::model::tree_like::TreeLike;
use crate::LbServer;
use uuid::Uuid;

impl LbServer {
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn create_link_at_path(&self, path: &str, target_id: Uuid) -> LbResult<File> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();

        let root = db.root.get().ok_or(LbErrKind::RootNonexistent)?;

        let id = tree.create_link_at_path(path, target_id, root, &self.keychain)?;

        let ui_file = tree.decrypt(&self.keychain, &id, &db.pub_key_lookup)?;

        self.events.meta_changed();

        Ok(ui_file)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn create_at_path(&self, path: &str) -> LbResult<File> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();

        let root = db.root.get().ok_or(LbErrKind::RootNonexistent)?;

        let id = tree.create_at_path(path, root, &self.keychain)?;

        let ui_file = tree.decrypt(&self.keychain, &id, &db.pub_key_lookup)?;

        self.events.meta_changed();

        Ok(ui_file)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_by_path(&self, path: &str) -> LbResult<File> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let root = db.root.get().ok_or(LbErrKind::RootNonexistent)?;

        let id = tree.path_to_id(path, root, &self.keychain)?;

        let ui_file = tree.decrypt(&self.keychain, &id, &db.pub_key_lookup)?;

        Ok(ui_file)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_path_by_id(&self, id: Uuid) -> LbResult<String> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let path = tree.id_to_path(&id, &self.keychain)?;

        Ok(path)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_paths(&self, filter: Option<Filter>) -> LbResult<Vec<String>> {
        Ok(self
            .list_paths_with_ids(filter)
            .await?
            .into_iter()
            .map(|(_, path)| path)
            .collect())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_paths_with_ids(
        &self, filter: Option<Filter>,
    ) -> LbResult<Vec<(Uuid, String)>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let paths = tree.list_paths(filter, &self.keychain)?;

        Ok(paths)
    }
}
