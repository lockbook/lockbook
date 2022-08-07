use crate::OneKey;
use crate::{CoreError, CoreResult, RequestContext};
use lockbook_shared::file::File;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::lazy::LazyStaged1;
use lockbook_shared::path_ops::Filter;
use lockbook_shared::tree_like::Stagable;
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn create_link_at_path(&mut self, path: &str, target_id: Uuid) -> CoreResult<File> {
        todo!()
    }

    pub fn create_at_path(&mut self, path: &str) -> CoreResult<File> {
        let pub_key = self.get_public_key()?;
        let mut tree = LazyStaged1::core_tree(
            Owner(self.get_public_key()?),
            &mut self.tx.base_metadata,
            &mut self.tx.local_metadata,
        );
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

        let (mut tree, id) = tree.create_at_path(path, root, account, &pub_key)?;

        let ui_file = tree.finalize(&id, account)?;

        Ok(ui_file)
    }

    pub fn get_by_path(&mut self, path: &str) -> CoreResult<File> {
        let mut tree = LazyStaged1::core_tree(
            Owner(self.get_public_key()?),
            &mut self.tx.base_metadata,
            &mut self.tx.local_metadata,
        );
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

        let ui_file = tree.finalize(&id, account)?;

        Ok(ui_file)
    }

    pub fn get_path_by_id(&mut self, id: Uuid) -> CoreResult<String> {
        let mut tree = LazyStaged1::core_tree(
            Owner(self.get_public_key()?),
            &mut self.tx.base_metadata,
            &mut self.tx.local_metadata,
        );
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let path = tree.id_to_path(&id, account)?;

        Ok(path)
    }

    pub fn list_paths(&mut self, filter: Option<Filter>) -> Result<Vec<String>, CoreError> {
        let mut tree = LazyStaged1::core_tree(
            Owner(self.get_public_key()?),
            &mut self.tx.base_metadata,
            &mut self.tx.local_metadata,
        );
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;
        let paths = tree.list_paths(filter, account)?;

        Ok(paths)
    }
}
