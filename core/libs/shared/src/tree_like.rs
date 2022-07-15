use crate::account::Account;
use crate::crypto::AESKey;
use crate::file_like::FileLike;
use crate::lazy_file::LazyFile;
use crate::tree_like::TreeError::*;
use crate::{pubkey, symkey};
use std::collections::HashSet;
use std::marker::PhantomData;
use uuid::Uuid;

pub trait TreeLike<F: FileLike> {
    fn ids(&self) -> HashSet<Uuid>;
    fn maybe_find(&self, id: Uuid) -> Option<&LazyFile<F>>;
    fn maybe_find_mut(&mut self, id: Uuid) -> Option<&mut LazyFile<F>>;

    fn find(&self, id: Uuid) -> Result<&LazyFile<F>, TreeError> {
        self.maybe_find(id).ok_or(FileNonexistent)
    }

    fn find_mut(&mut self, id: Uuid) -> Result<&mut LazyFile<F>, TreeError> {
        self.maybe_find_mut(id).ok_or(FileNonexistent)
    }

    fn maybe_find_parent<F2: FileLike>(&self, file: &F2) -> Option<&LazyFile<F>> {
        self.maybe_find(file.parent())
    }

    fn find_parent<F2: FileLike>(&self, file: &F2) -> Result<&LazyFile<F>, TreeError> {
        self.maybe_find_parent(file).ok_or(FileParentNonexistent)
    }

    fn find_parent_mut<F2: FileLike>(&mut self, file: &F2) -> Result<&mut LazyFile<F>, TreeError> {
        self.maybe_find_mut(file.parent())
            .ok_or(FileParentNonexistent)
    }

    fn calculate_deleted(&mut self, id: Uuid) -> Result<bool, TreeError> {
        let mut file = self.find(id)?;
        let mut visited_ids = vec![];
        let mut deleted = false;

        while !file.is_root() {
            visited_ids.push(file.id());
            if let Some(implicit) = file.implicitly_deleted {
                deleted = implicit;
                break;
            }

            if file.file.explicitly_deleted() {
                deleted = true;
                break;
            }

            file = self.find_parent(file)?;
        }

        for id in visited_ids {
            self.find_mut(id)?.implicitly_deleted = Some(deleted);
        }

        Ok(deleted)
    }

    fn decrypt_key(&mut self, id: Uuid, account: &Account) -> Result<AESKey, TreeError> {
        let mut file = self.find(id)?;
        let mut visited_ids = vec![];

        loop {
            if file.key.is_some() {
                break;
            }

            if let Some(user_access) = file.user_access_keys().get(&account.username) {
                let user_access_key =
                    pubkey::get_aes_key(&account.private_key, &user_access.encrypted_by).unwrap();
                let file_key =
                    Some(symkey::decrypt(&user_access_key, &user_access.access_key).unwrap());
                let id = file.id();
                self.find_mut(id)?.key = file_key;
                break;
            }

            visited_ids.push(file.id());
            file = self.find_parent(file)?;
        }

        for id in visited_ids.iter().rev() {
            let meta = self.find(*id)?;
            let parent = self.find_parent(meta)?;
            let parent_key = parent.key.unwrap();
            let encrypted_key = meta.folder_access_keys();
            let decrypted_key = symkey::decrypt(&parent_key, encrypted_key).unwrap();
            self.find_mut(*id)?.key = Some(decrypted_key)
        }

        Ok(self.find_mut(id)?.key.unwrap())
    }

    fn name(&mut self, id: Uuid, account: &Account) -> Result<String, TreeError> {
        let meta = self.find(id)?;
        if let Some(name) = &meta.name {
            return Ok(name.clone());
        }

        let parent_id = meta.parent();
        let parent_key = self.decrypt_key(parent_id, account)?;

        let meta = self.find_mut(id)?;
        let name = meta.secret_name().to_string(&parent_key).unwrap();
        meta.name = Some(name.clone());
        Ok(name)
    }

    fn stage<Staged>(self, staged: Staged) -> StagedTree<F, Self, Staged>
    where
        Staged: TreeLike<F>,
        Self: Sized,
    {
        StagedTree::new(self, staged)
    }
}

pub struct StagedTree<F, Base, Local>
where
    F: FileLike,
    Base: TreeLike<F>,
    Local: TreeLike<F>,
{
    base: Base,
    local: Local,
    _f: PhantomData<F>,
}

impl<F, Base, Local> StagedTree<F, Base, Local>
where
    F: FileLike,
    Base: TreeLike<F>,
    Local: TreeLike<F>,
{
    pub fn new(base: Base, local: Local) -> Self {
        Self { base, local, _f: Default::default() }
    }
}

impl<F, Base, Local> TreeLike<F> for StagedTree<F, Base, Local>
where
    F: FileLike,
    Base: TreeLike<F>,
    Local: TreeLike<F>,
{
    fn ids(&self) -> HashSet<Uuid> {
        let mut ids = self.base.ids();
        ids.extend(self.local.ids());
        ids
    }

    fn maybe_find(&self, id: Uuid) -> Option<&LazyFile<F>> {
        self.local
            .maybe_find(id)
            .or_else(|| self.base.maybe_find(id))
    }

    fn maybe_find_mut(&mut self, id: Uuid) -> Option<&mut LazyFile<F>> {
        let local = self.local.maybe_find_mut(id);
        match local {
            Some(lf) => Some(lf),
            None => self.base.maybe_find_mut(id),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeError {
    RootNonexistent,
    FileNonexistent,
    FileParentNonexistent,
    Unexpected(String),
}
