use crate::{CoreError, CoreResult, OneKey, RequestContext};
use lockbook_shared::file::File;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileType, Owner};
use lockbook_shared::tree_like::{Stagable, TreeLike};
use std::collections::HashMap;
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

        let mut parent_substitutions = HashMap::new();

        for id in tree.owned_ids() {
            if !tree.calculate_deleted(&id)? {
                let finalized = tree.finalize(&id, account)?;
                match finalized.file_type {
                    FileType::Document | FileType::Folder => files.push(finalized),
                    FileType::Link { target } => {
                        let mut target_file = tree.finalize(&target, account)?;
                        if target_file.is_folder() {
                            parent_substitutions.insert(target, id);
                        }

                        target_file.id = finalized.id;
                        target_file.parent = finalized.parent;
                        target_file.name = finalized.name;

                        files.push(target_file);
                    }
                }
            }
        }

        for item in &mut files {
            if let Some(new_parent) = parent_substitutions.get(&item.id) {
                item.parent = *new_parent;
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

        let id = match tree.find(id)?.file_type() {
            FileType::Document | FileType::Folder => *id,
            FileType::Link { target } => target,
        };

        let mut children = Vec::new();

        for id in tree.children(&id)? {
            children.push(tree.finalize(&id, account)?);
        }

        Ok(children)
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

        let id = match tree.find(id)?.file_type() {
            FileType::Document | FileType::Folder => *id,
            FileType::Link { target } => target,
        };

        let mut files = vec![tree.finalize(&id, account)?];

        for id in tree.descendants(&id)? {
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
