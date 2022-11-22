use crate::{CoreError, CoreResult, RequestContext};
use crate::{OneKey, Requester};
use lockbook_shared::file::File;
use lockbook_shared::path_ops::Filter;
use lockbook_shared::tree_like::{TreeLike, TreeLikeMut};
use uuid::Uuid;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn create_link_at_path(&mut self, path: &str, target_id: Uuid) -> CoreResult<File> {
        let pub_key = self.get_public_key()?;
        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let root = self
            .tx
            .root
            .get(&OneKey {})
            .ok_or(CoreError::RootNonexistent)?;

        let id = tree.create_link_at_path(path, target_id, root, account, &pub_key)?;
        let ui_file = tree.finalize(&id, account, &mut self.tx.username_by_public_key)?;

        Ok(ui_file)
    }

    pub fn create_at_path(&mut self, path: &str) -> CoreResult<File> {
        let pub_key = self.get_public_key()?;
        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let root = self
            .tx
            .root
            .get(&OneKey {})
            .ok_or(CoreError::RootNonexistent)?;

        let id = tree.create_at_path(path, root, account, &pub_key)?;
        let ui_file = tree.finalize(&id, account, &mut self.tx.username_by_public_key)?;

        Ok(ui_file)
    }

    pub fn get_by_path(&mut self, path: &str) -> CoreResult<File> {
        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let root = self
            .tx
            .root
            .get(&OneKey {})
            .ok_or(CoreError::RootNonexistent)?;

        let id = tree.path_to_id(path, root, account)?;

        let ui_file = tree.finalize(&id, account, &mut self.tx.username_by_public_key)?;

        Ok(ui_file)
    }

    pub fn get_path_by_id(&mut self, id: Uuid) -> CoreResult<String> {
        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let path = tree.id_to_path(&id, account)?;

        Ok(path)
    }

    pub fn list_paths(&mut self, filter: Option<Filter>) -> CoreResult<Vec<String>> {
        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let paths = tree.list_paths(filter, account)?;

        Ok(paths)
    }
}
