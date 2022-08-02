use crate::{CoreError, OneKey, RequestContext};
use lockbook_shared::file::File;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn create_file(
        &mut self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> Result<File, CoreError> {
        let pub_key = self.get_public_key()?;
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let (mut tree, id) = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .create(parent, name, file_type, account, &pub_key)?;

        let ui_file = tree.finalize(&id, account)?;

        Ok(ui_file)
    }

    pub fn rename_file(&mut self, id: &Uuid, new_name: &str) -> Result<(), CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        self.tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .rename(id, new_name, account)?;

        Ok(())
    }

    pub fn move_file(&mut self, id: &Uuid, new_parent: &Uuid) -> Result<(), CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        self.tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .move_file(id, new_parent, account)?;
        Ok(())
    }

    pub fn delete(&mut self, id: &Uuid) -> Result<(), CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .delete(id, account)?;

        for id in tree.prunable_ids()? {
            tree.remove(id);
        }

        Ok(())
    }

    pub fn root(&mut self) -> Result<File, CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let root_id = self
            .tx
            .root
            .get(&OneKey {})
            .ok_or(CoreError::RootNonexistent)?;

        let root = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .finalize(root_id, account)?;

        Ok(root)
    }

    pub fn list_metadatas(&mut self) -> Result<Vec<File>, CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();

        let mut files = Vec::new();

        for id in tree.owned_ids() {
            files.push(tree.finalize(&id, account)?);
        }

        Ok(files)
    }

    pub fn get_children(&mut self, id: &Uuid) -> Result<Vec<File>, CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();

        let mut children = Vec::new();

        for id in tree.children(id)? {
            children.push(tree.finalize(&id, account)?);
        }

        Ok(children)
    }

    pub fn get_and_get_children(&mut self, id: &Uuid) -> Result<Vec<File>, CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();

        let mut files = Vec::new();
        files.push(tree.finalize(&id, account)?);

        for id in tree.children(id)? {
            files.push(tree.finalize(&id, account)?);
        }

        Ok(files)
    }

    pub fn get_file_by_id(&mut self, id: &Uuid) -> Result<File, CoreError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();

        Ok(tree.finalize(id, account)?)
    }
}
