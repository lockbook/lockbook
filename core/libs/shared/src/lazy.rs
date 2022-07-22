use crate::account::Account;
use crate::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::file::File;
use crate::file_like::FileLike;
use crate::signed_file::SignedFile;
use crate::staged::StagedTree;
use crate::tree_like::{Stagable, TreeLike};
use crate::{pubkey, symkey, SharedError, SharedResult};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use uuid::Uuid;

pub struct LazyTree<F: FileLike, T: Stagable<F>> {
    tree: T,
    name_by_id: HashMap<Uuid, String>,
    key_by_id: HashMap<Uuid, AESKey>,
    implicitly_deleted_by_id: HashMap<Uuid, bool>,
    _f: PhantomData<F>,
}

impl<F: FileLike, T: Stagable<F>> LazyTree<F, T> {
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

impl<F: FileLike, T: Stagable<F>> LazyTree<F, T> {
    pub fn calculate_deleted(&mut self, id: Uuid) -> SharedResult<bool> {
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

    pub fn decrypt_key(&mut self, id: Uuid, account: &Account) -> SharedResult<AESKey> {
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
                    pubkey::get_aes_key(&account.private_key, &user_access.encrypted_by)?;
                let file_key = symkey::decrypt(&user_access_key, &user_access.access_key)?;
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
                let parent_key =
                    self.key_by_id
                        .get(&parent.id())
                        .ok_or(SharedError::Unexpected(
                            "parent key should have been populated by prior routine",
                        ))?;
                let encrypted_key = file.folder_access_keys();
                symkey::decrypt(parent_key, encrypted_key)?
            };
            self.key_by_id.insert(*id, decrypted_key);
        }

        Ok(*self.key_by_id.get(&id).ok_or(SharedError::Unexpected(
            "parent key should have been populated by prior routine (2)",
        ))?)
    }

    pub fn name(&mut self, id: Uuid, account: &Account) -> SharedResult<String> {
        if let Some(name) = self.name_by_id.get(&id) {
            return Ok(name.clone());
        }

        let parent_id = self.find(id)?.parent();
        let parent_key = self.decrypt_key(parent_id, account)?;

        let name = self.find(id)?.secret_name().to_string(&parent_key)?;
        self.name_by_id.insert(id, name.clone());
        Ok(name)
    }

    pub fn encrypt_document(
        &mut self, id: Uuid, document: &DecryptedDocument, account: &Account,
    ) -> SharedResult<EncryptedDocument> {
        let key = self.decrypt_key(id, account)?;
        symkey::encrypt(&key, document)
    }

    pub fn decrypt_document(
        &mut self, id: Uuid, encrypted: &EncryptedDocument, account: &Account,
    ) -> SharedResult<DecryptedDocument> {
        let key = self.decrypt_key(id, account)?;
        symkey::decrypt(&key, encrypted)
    }

    pub fn finalize(&mut self, id: Uuid, account: &Account) -> SharedResult<File> {
        let meta = self.find(id)?;
        let file_type = meta.file_type();
        let parent = meta.parent();
        let name = self.name(id, account)?;
        Ok(File { id, parent, name, file_type })
    }

    pub fn stage<T2: Stagable<F>>(self, staged: T2) -> LazyTree<F, StagedTree<F, T, T2>> {
        todo!()
    }

    pub fn validate(&mut self) -> SharedResult<()> {
        todo!()
    }
}

impl<Base: Stagable<SignedFile>, Local: Stagable<SignedFile>>
    LazyTree<SignedFile, StagedTree<SignedFile, Base, Local>>
{
    pub fn get_changes(&self) -> SharedResult<Vec<&SignedFile>> {
        let base = self.tree.base.ids();
        let local = self.tree.staged.ids();
        let exists_both = local.iter().filter(|id| base.contains(id));

        let mut changed = vec![];

        for id in exists_both {
            let base = self.tree.base.find(*id)?;
            let local = self.tree.staged.find(*id)?;
            if *local == *base {
                changed.push(local);
            }
        }

        Ok(changed)
    }
}

pub type Tree<F, T> = LazyTree<F, T>;
pub type Stage1<F, Base, Local> = StagedTree<F, Base, Local>;
pub type LazyStaged1<F, Base, Local> = LazyTree<F, Stage1<F, Base, Local>>;
pub type Stage2<F, Base, Local, Staged> = StagedTree<F, StagedTree<F, Base, Local>, Staged>;
pub type LazyStage2<F, Base, Local, Staged> = Tree<F, Stage2<F, Base, Local, Staged>>;

impl<F, Base, Local, Staged> LazyStage2<F, Base, Local, Staged>
where
    F: FileLike,
    Base: Stagable<F>,
    Local: Stagable<F>,
    Staged: Stagable<F>,
{
    pub fn promote(self) -> LazyStaged1<F, Base, Local> {
        let mut staged = self.tree.staged;
        let mut base = self.tree.base;
        for id in staged.ids() {
            if let Some(removed) = staged.remove(id) {
                base.insert(removed);
            }
        }

        LazyStaged1 {
            tree: base,
            name_by_id: self.name_by_id,
            key_by_id: self.key_by_id,
            implicitly_deleted_by_id: self.implicitly_deleted_by_id,
            _f: Default::default(),
        }
    }
}

impl<F: FileLike, T: Stagable<F>> TreeLike<F> for LazyTree<F, T> {
    fn ids(&self) -> HashSet<Uuid> {
        self.tree.ids()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&F> {
        self.tree.maybe_find(id)
    }

    fn insert(&mut self, f: F) -> Option<F> {
        self.tree.insert(f)
    }

    fn remove(&mut self, id: Uuid) -> Option<F> {
        self.tree.remove(id)
    }
}
