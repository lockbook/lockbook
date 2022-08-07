use crate::OneKey;
use crate::{CoreError, CoreResult, RequestContext};
use lockbook_shared::file::File;
use lockbook_shared::path_ops::Filter;
use lockbook_shared::tree_like::Stagable;
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn create_at_path(&mut self, path: &str) -> CoreResult<File> {
        let pub_key = self.get_public_key()?;
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

        let (mut tree, id) = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .create_at_path(path, root, account, &pub_key)?;

        let ui_file = tree.finalize(&id, account)?;

        Ok(ui_file)
    }

    pub fn get_by_path(&mut self, path: &str) -> CoreResult<File> {
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

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();

        let id = tree.path_to_id(path, root, account)?;

        let ui_file = tree.finalize(&id, account)?;

        Ok(ui_file)
    }

    pub fn get_path_by_id(&mut self, id: Uuid) -> CoreResult<String> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let path = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .id_to_path(&id, account)?;

        Ok(path)
    }

    pub fn list_paths(&mut self, filter: Option<Filter>) -> Result<Vec<String>, CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let paths = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .list_paths(filter, account)?;

        Ok(paths)
    }
}
