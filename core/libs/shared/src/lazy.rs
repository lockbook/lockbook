use crate::account::Account;
use crate::crypto::AESKey;
use crate::file_like::FileLike;
use crate::tree_like::{TreeError, TreeLike};
use crate::{pubkey, symkey};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use uuid::Uuid;

pub struct LazyTree<F: FileLike, T: TreeLike<F>> {
    tree: T,
    name_by_id: HashMap<Uuid, String>,
    key_by_id: HashMap<Uuid, AESKey>,
    implicitly_deleted_by_id: HashMap<Uuid, bool>,
    _f: PhantomData<F>,
}

impl<F: FileLike, T: TreeLike<F>> LazyTree<F, T> {
    pub fn new(tree: T) -> Self {
        Self {
            tree,
            name_by_id: HashMap::new(),
            key_by_id: HashMap::new(),
            implicitly_deleted_by_id: HashMap::new(),
            _f: Default::default(),
        }
    }
}

impl<F: FileLike, T: TreeLike<F>> LazyTree<F, T> {
    pub fn calculate_deleted(&mut self, id: Uuid) -> Result<bool, TreeError> {
        let (visited_ids, deleted) = {
            let mut file = self.find(id)?;
            let mut visited_ids = vec![];
            let mut deleted = false;

            while !file.is_root() {
                visited_ids.push(file.id());
                if let Some(&implicit) = self.implicitly_deleted_by_id.get(&file.id()) {
                    deleted = implicit;
                    break;
                }

                if file.explicitly_deleted() {
                    deleted = true;
                    break;
                }

                file = self.find_parent(&file)?;
            }

            (visited_ids, deleted)
        };

        for id in visited_ids {
            self.implicitly_deleted_by_id.insert(id, deleted);
        }

        Ok(deleted)
    }

    pub fn decrypt_key(&mut self, id: Uuid, account: &Account) -> Result<AESKey, TreeError> {
        let mut file_id = self.find(id)?.id();
        let mut visited_ids = vec![];

        loop {
            if self.key_by_id.get(&file_id).is_some() {
                break;
            }

            let maybe_file_key = if let Some(user_access) = self
                .find(file_id)?
                .user_access_keys()
                .get(&account.username)
            {
                let user_access_key =
                    pubkey::get_aes_key(&account.private_key, &user_access.encrypted_by).unwrap();
                let file_key = symkey::decrypt(&user_access_key, &user_access.access_key).unwrap();
                Some(file_key)
            } else {
                None
            };
            if let Some(file_key) = maybe_file_key {
                self.key_by_id.insert(file_id, file_key);
                break;
            }

            visited_ids.push(file_id);
            file_id = self.find_parent(&self.find(file_id)?)?.id();
        }

        for id in visited_ids.iter().rev() {
            let decrypted_key = {
                let file = self.find(*id)?;
                let parent = self.find_parent(&file)?;
                let parent_key = self.key_by_id.get(&parent.id()).unwrap();
                let encrypted_key = file.folder_access_keys();
                let decrypted_key = symkey::decrypt(&parent_key, encrypted_key).unwrap();
                decrypted_key
            };
            self.key_by_id.insert(*id, decrypted_key);
        }

        Ok(*self.key_by_id.get(&id).unwrap())
    }

    pub fn name(&mut self, id: Uuid, account: &Account) -> Result<String, TreeError> {
        if let Some(name) = self.name_by_id.get(&id) {
            return Ok(name.clone());
        }

        let parent_id = self.find(id)?.parent();
        let parent_key = self.decrypt_key(parent_id, account)?;

        let name = self.find(id)?.secret_name().to_string(&parent_key).unwrap();
        self.name_by_id.insert(id, name.clone());
        Ok(name)
    }
}

impl<F: FileLike, T: TreeLike<F>> TreeLike<F> for LazyTree<F, T> {
    fn ids(&self) -> HashSet<Uuid> {
        self.tree.ids()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&F> {
        self.tree.maybe_find(id)
    }
}
