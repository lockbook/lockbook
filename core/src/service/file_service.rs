use crate::{CoreError, OneKey, RequestContext};
use lockbook_shared::file::File;
use lockbook_shared::file_metadata::{FileType, Owner};
use lockbook_shared::lazy::LazyStaged1;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn create_file(
        &mut self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> Result<File, CoreError> {
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

        let (mut tree, id) = tree.create(parent, name, file_type, account, &pub_key)?;

        let ui_file = tree.finalize(&id, account)?;

        Ok(ui_file)
    }

    pub fn rename_file(&mut self, id: &Uuid, new_name: &str) -> Result<(), CoreError> {
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

        tree.rename(id, new_name, account)?;

        Ok(())
    }

    pub fn move_file(&mut self, id: &Uuid, new_parent: &Uuid) -> Result<(), CoreError> {
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

        tree.move_file(id, new_parent, account)?;
        Ok(())
    }

    pub fn delete(&mut self, id: &Uuid) -> Result<(), CoreError> {
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

        let tree = tree.delete(id, account)?;

        let (mut tree, prunable_ids) = tree.prunable_ids()?;
        for id in prunable_ids {
            tree.remove(id);
        }

        Ok(())
    }

    pub fn root(&mut self) -> Result<File, CoreError> {
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

        let root_id = self
            .tx
            .root
            .get(&OneKey {})
            .ok_or(CoreError::RootNonexistent)?;

        let root = tree.finalize(root_id, account)?;

        Ok(root)
    }

    pub fn list_metadatas(&mut self) -> Result<Vec<File>, CoreError> {
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

        let mut files = Vec::new();

        for id in tree.owned_ids() {
            if !tree.calculate_deleted(&id)? {
                files.push(tree.finalize(&id, account)?);
            }
        }

        Ok(files)
    }

    pub fn get_children(&mut self, id: &Uuid) -> Result<Vec<File>, CoreError> {
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

        let mut children = Vec::new();

        for id in tree.children(id)? {
            children.push(tree.finalize(&id, account)?);
        }

        Ok(children)
    }

    pub fn get_and_get_children(&mut self, id: &Uuid) -> Result<Vec<File>, CoreError> {
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

        let mut files = vec![tree.finalize(id, account)?];

        for id in tree.children(id)? {
            files.push(tree.finalize(&id, account)?);
        }

        Ok(files)
    }

    pub fn get_file_by_id(&mut self, id: &Uuid) -> Result<File, CoreError> {
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

        Ok(tree.finalize(id, account)?)
    }
}
