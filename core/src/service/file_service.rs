use crate::{CoreError, CoreResult, OneKey, RequestContext, Requester};
use lockbook_shared::file::File;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileType, Owner};
use lockbook_shared::tree_like::{Stagable, TreeLike};
use std::iter;
use uuid::Uuid;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn create_file(
        &mut self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> CoreResult<File> {
        let pub_key = self.get_public_key()?;
        let tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let (mut tree, id) = tree.create(parent, name, file_type, account, &pub_key)?;

        let ui_file = tree.finalize(&id, account)?;

        info!("created {:?} with id {id}", file_type);

        Ok(ui_file)
    }

    pub fn rename_file(&mut self, id: &Uuid, new_name: &str) -> CoreResult<()> {
        let tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        tree.rename(id, new_name, account)?;

        Ok(())
    }

    pub fn move_file(&mut self, id: &Uuid, new_parent: &Uuid) -> CoreResult<()> {
        let tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        tree.move_file(id, new_parent, account)?;
        Ok(())
    }

    pub fn delete(&mut self, id: &Uuid) -> CoreResult<()> {
        let tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        tree.delete(id, account)?;

        Ok(())
    }

    pub fn root(&mut self) -> CoreResult<File> {
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

        let root_id = self
            .tx
            .root
            .get(&OneKey {})
            .ok_or(CoreError::RootNonexistent)?;

        let root = tree.finalize(root_id, account)?;

        Ok(root)
    }

    pub fn list_metadatas(&mut self) -> CoreResult<Vec<File>> {
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

        let ids = tree.owned_ids().into_iter();

        Ok(tree.resolve_and_finalize(account, ids)?)
    }

    pub fn get_children(&mut self, id: &Uuid) -> CoreResult<Vec<File>> {
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

        let ids = tree.children_using_links(id)?.into_iter();
        Ok(tree.resolve_and_finalize(account, ids)?)
    }

    pub fn get_and_get_children_recursively(&mut self, id: &Uuid) -> CoreResult<Vec<File>> {
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

        let descendants = tree.descendants_using_links(id)?;
        Ok(tree.resolve_and_finalize(account, descendants.into_iter().chain(iter::once(*id)))?)
    }

    pub fn get_file_by_id(&mut self, id: &Uuid) -> CoreResult<File> {
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
        if tree.calculate_deleted(id)? {
            return Err(CoreError::FileNonexistent);
        }

        Ok(tree.finalize(id, account)?)
    }

    pub fn find_owner(&self, id: &Uuid) -> CoreResult<Owner> {
        let meta = match self.tx.base_metadata.get(id) {
            Some(file) => file,
            None => self
                .tx
                .local_metadata
                .get(id)
                .ok_or(CoreError::FileNonexistent)?,
        };

        Ok(meta.owner())
    }
}
