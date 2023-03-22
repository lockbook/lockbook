use crate::{CoreError, CoreResult};
use crate::{CoreState, Requester};
use lockbook_shared::file::File;
use lockbook_shared::path_ops::Filter;
use lockbook_shared::tree_like::TreeLike;
use uuid::Uuid;

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn create_link_at_path(&mut self, path: &str, target_id: Uuid) -> CoreResult<File> {
        let pub_key = self.get_public_key()?;
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let root = self.db.root.data().ok_or(CoreError::RootNonexistent)?;

        let id = tree.create_link_at_path(path, target_id, root, account, &pub_key)?;

        let mut ui_file = tree.resolve_and_finalize(account, id, &mut self.db.pub_key_lookup)?;
        ui_file.id = id;

        Ok(ui_file)
    }

    pub(crate) fn create_at_path(&mut self, path: &str) -> CoreResult<File> {
        let pub_key = self.get_public_key()?;
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let root = self.db.root.data().ok_or(CoreError::RootNonexistent)?;

        let id = tree.create_at_path(path, root, account, &pub_key)?;

        let ui_file = tree.resolve_and_finalize(account, id, &mut self.db.pub_key_lookup)?;

        Ok(ui_file)
    }

    pub(crate) fn get_by_path(&mut self, path: &str) -> CoreResult<File> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let root = self.db.root.data().ok_or(CoreError::RootNonexistent)?;

        let id = tree.path_to_id(path, root, account)?;

        let ui_file = tree.resolve_and_finalize(account, id, &mut self.db.pub_key_lookup)?;

        Ok(ui_file)
    }

    pub(crate) fn get_path_by_id(&mut self, id: Uuid) -> CoreResult<String> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;
        let path = tree.id_to_path(&id, account)?;

        Ok(path)
    }

    pub(crate) fn list_paths(&mut self, filter: Option<Filter>) -> CoreResult<Vec<String>> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;
        let paths = tree.list_paths(filter, account)?;

        Ok(paths)
    }
}
