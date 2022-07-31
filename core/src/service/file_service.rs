use crate::model::repo::RepoSource;
use crate::repo::document_repo;
use crate::{Config, CoreError, OneKey, RequestContext};
use itertools::Itertools;
use lockbook_shared::compression_service;
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::crypto::EncryptedDocument;
use lockbook_shared::file::File;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::lazy::LazyTree;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use sha2::Digest;
use sha2::Sha256;
use std::collections::{HashMap, HashSet};
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

        self.tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .delete(id, account)?;
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

        let mut files = Vec::new();

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();

        for id in tree.owned_ids() {
            files.push(tree.finalize(&id, account)?);
        }

        Ok(files)
    }
}
