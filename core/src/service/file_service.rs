use crate::{CoreError, CoreResult, OneKey, RequestContext};
use lockbook_shared::file::File;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileType, Owner};
use lockbook_shared::tree_like::{Stagable, TreeLike};
use uuid::Uuid;

impl RequestContext<'_, '_> {
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

        let tree = tree.delete(id, account)?;

        let (mut tree, prunable_ids) = tree.prunable_ids()?;
        for id in prunable_ids {
            tree.remove(id);
        }

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

        let mut files = Vec::new();

        for id in tree.owned_ids() {
            if !tree.calculate_deleted(&id)? && !tree.in_pending_share(&id)? {
                let (id, parent) = {
                    let file = tree.find(&id)?;

                    match file.file_type() {
                        FileType::Link { target } => {
                            let resolved_file = tree.find(&target)?;

                            (*resolved_file.id(), *file.parent())
                        }
                        _ => (*file.id(), *file.parent()),
                    }
                };

                let mut file = tree.finalize(&id, account)?;
                if file.parent != parent {
                    file.parent = parent;
                }

                files.push(file);
            }
        }

        Ok(files)
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

        let mut children = Vec::new();

        for id in tree.children(id)? {
            if !tree.calculate_deleted(&id)? && !tree.in_pending_share(&id)? {
                let (id, parent) = {
                    let file = tree.find(&id)?;

                    match file.file_type() {
                        FileType::Link { target } => {
                            let resolved_file = tree.find(&target)?;

                            (*resolved_file.id(), *file.parent())
                        }
                        _ => (*file.id(), *file.parent()),
                    }
                };

                let mut file = tree.finalize(&id, account)?;
                if file.parent != parent {
                    file.parent = parent;
                }

                children.push(file);
            }
        }

        Ok(children)
    }

    pub fn get_and_get_children(&mut self, id: &Uuid) -> CoreResult<Vec<File>> {
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

        let mut files = vec![tree.finalize(id, account)?];

        for id in tree.children(id)? {
            files.push(tree.finalize(&id, account)?);
        }

        Ok(files)
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
