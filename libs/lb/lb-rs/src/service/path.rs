use crate::logic::path_ops::Filter;
use crate::logic::tree_like::TreeLike;
use crate::model::errors::{CoreError, LbResult};
use crate::model::file::File;
use crate::Lb;
use uuid::Uuid;

impl Lb {
    pub async fn create_link_at_path(&self, path: &str, target_id: Uuid) -> LbResult<File> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let pub_key = self.get_pk()?;
        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        let account = self.get_account()?;

        let root = db.root.get().ok_or(CoreError::RootNonexistent)?;

        let id = tree.create_link_at_path(path, target_id, root, account, &pub_key)?;

        let ui_file = tree.decrypt(account, &id, &db.pub_key_lookup)?;

        Ok(ui_file)
    }

    pub async fn create_at_path(&self, path: &str) -> LbResult<File> {
        let pub_key = self.get_pk()?;
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        let account = self.get_account()?;

        let root = db.root.get().ok_or(CoreError::RootNonexistent)?;

        let id = tree.create_at_path(path, root, account, &pub_key)?;

        let ui_file = tree.decrypt(account, &id, &db.pub_key_lookup)?;

        Ok(ui_file)
    }

    pub async fn get_by_path(&self, path: &str) -> LbResult<File> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let account = self.get_account()?;

        let root = db.root.get().ok_or(CoreError::RootNonexistent)?;

        let id = tree.path_to_id(path, root, account)?;

        let ui_file = tree.decrypt(account, &id, &db.pub_key_lookup)?;

        Ok(ui_file)
    }

    pub async fn get_path_by_id(&self, id: Uuid) -> LbResult<String> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let account = self.get_account()?;
        let path = tree.id_to_path(&id, account)?;

        Ok(path)
    }

    pub async fn list_paths(&self, filter: Option<Filter>) -> LbResult<Vec<String>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let account = self.get_account()?;
        let paths = tree.list_paths(filter, account)?;

        Ok(paths)
    }
}
