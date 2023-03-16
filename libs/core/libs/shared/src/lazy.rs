use crate::access_info::UserAccessMode;
use crate::account::Account;
use crate::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::file_like::FileLike;
use crate::file_metadata::{FileType, Owner};
use crate::staged::StagedTree;
use crate::tree_like::{TreeLike, TreeLikeMut};
use crate::{compression_service, symkey, SharedErrorKind, SharedResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug)]
pub struct LazyTree<T: TreeLike> {
    pub tree: T,
    pub name: HashMap<Uuid, String>,
    pub key: HashMap<Uuid, AESKey>,
    pub implicit_deleted: HashMap<Uuid, bool>,
    pub children: HashMap<Uuid, HashSet<Uuid>>,
}

impl<T: TreeLike> LazyTree<T> {
    pub fn new(tree: T) -> Self {
        Self {
            name: HashMap::new(),
            key: HashMap::new(),
            implicit_deleted: HashMap::new(),
            children: HashMap::new(),
            tree,
        }
    }
}

impl<T: TreeLike> LazyTree<T> {
    pub fn access_mode(&self, owner: Owner, id: &Uuid) -> SharedResult<Option<UserAccessMode>> {
        let mut file = self.find(id)?;
        let mut max_access_mode = None;
        let mut visited_ids = vec![];
        while !visited_ids.contains(file.id()) {
            visited_ids.push(*file.id());
            let access_mode = file.access_mode(&owner);
            if access_mode > max_access_mode {
                max_access_mode = access_mode;
            }
            if file.parent() == file.id() {
                break; // root
            } else if let Some(parent) = self.maybe_find(file.parent()) {
                file = parent
            } else {
                break; // share root
            }
        }
        Ok(max_access_mode)
    }

    pub fn in_pending_share(&mut self, id: &Uuid) -> SharedResult<bool> {
        let mut id = *id;
        loop {
            if self.find(&id)?.parent() == self.find(&id)?.id() {
                return Ok(false); // root
            } else if let Some(link) = self.link(&id)? {
                id = link;
            } else if self.maybe_find(self.find(&id)?.parent()).is_some() {
                id = *self.find(&id)?.parent();
            } else {
                return Ok(true); // share root
            }
        }
    }

    pub fn all_children(&mut self) -> SharedResult<&HashMap<Uuid, HashSet<Uuid>>> {
        if self.children.is_empty() {
            let mut all_children: HashMap<Uuid, HashSet<Uuid>> = HashMap::new();
            for file in self.all_files()? {
                if !file.is_root() {
                    let mut children = all_children.remove(file.parent()).unwrap_or_default();
                    children.insert(*file.id());
                    all_children.insert(*file.parent(), children);
                }
            }
            self.children = all_children;
        }

        Ok(&self.children)
    }

    pub fn calculate_deleted(&mut self, id: &Uuid) -> SharedResult<bool> {
        let (visited_ids, deleted) = {
            let mut file = self.find(id)?;
            let mut visited_ids = vec![];
            let mut deleted = false;

            while !file.is_root()
                && self.maybe_find(file.parent()).is_some()
                && !visited_ids.contains(file.id())
            {
                visited_ids.push(*file.id());
                if let Some(&implicit) = self.implicit_deleted.get(file.id()) {
                    deleted = implicit;
                    break;
                }

                if file.explicitly_deleted() {
                    deleted = true;
                    break;
                }

                file = match self.maybe_find_parent(file) {
                    Some(file) => file,
                    None => {
                        if !file.user_access_keys().is_empty() {
                            break;
                        } else {
                            return Err(SharedErrorKind::FileParentNonexistent.into());
                        }
                    }
                }
            }

            (visited_ids, deleted)
        };

        for id in visited_ids {
            self.implicit_deleted.insert(id, deleted);
        }

        Ok(deleted)
    }

    pub fn decrypt_key(&mut self, id: &Uuid, account: &Account) -> SharedResult<AESKey> {
        let mut file_id = *self.find(id)?.id();
        let mut visited_ids = vec![];

        loop {
            if self.key.get(&file_id).is_some() {
                break;
            }

            let my_pk = account.public_key();

            let maybe_file_key = if let Some(user_access) = self
                .find(&file_id)?
                .user_access_keys()
                .iter()
                .find(|access| access.encrypted_for == my_pk)
            {
                Some(user_access.decrypt(account)?)
            } else {
                None
            };
            if let Some(file_key) = maybe_file_key {
                self.key.insert(file_id, file_key);
                break;
            }

            visited_ids.push(file_id);
            file_id = *self.find_parent(self.find(&file_id)?)?.id();
        }

        for id in visited_ids.iter().rev() {
            let decrypted_key = {
                let file = self.find(id)?;
                let parent = self.find_parent(file)?;
                let parent_key = self
                    .key
                    .get(parent.id())
                    .ok_or(SharedErrorKind::Unexpected(
                        "parent key should have been populated by prior routine",
                    ))?;
                let encrypted_key = file.folder_access_key();
                symkey::decrypt(parent_key, encrypted_key)?
            };
            self.key.insert(*id, decrypted_key);
        }

        Ok(*self.key.get(id).ok_or(SharedErrorKind::Unexpected(
            "parent key should have been populated by prior routine (2)",
        ))?)
    }

    pub fn name(&mut self, id: &Uuid, account: &Account) -> SharedResult<String> {
        if let Some(name) = self.name.get(id) {
            return Ok(name.clone());
        }
        let key = self.decrypt_key(id, account)?;
        let name = self.find(id)?.secret_name().to_string(&key)?;
        self.name.insert(*id, name.clone());
        Ok(name)
    }

    pub fn name_using_links(&mut self, id: &Uuid, account: &Account) -> SharedResult<String> {
        let id = if let Some(link) = self.link(id)? { link } else { *id };
        self.name(&id, account)
    }

    pub fn link(&mut self, id: &Uuid) -> SharedResult<Option<Uuid>> {
        for link_id in self.owned_ids() {
            if let FileType::Link { target } = self.find(&link_id)?.file_type() {
                if id == &target && !self.calculate_deleted(&link_id)? {
                    return Ok(Some(link_id));
                }
            }
        }
        Ok(None)
    }

    /// Returns ids of files whose parent is the argument. Does not include the argument.
    /// TODO could consider returning a reference to the underlying cached value
    pub fn children(&mut self, id: &Uuid) -> SharedResult<HashSet<Uuid>> {
        // Check cache
        if let Some(children) = self.children.get(id) {
            return Ok(children.clone());
        }

        // Confirm file exists
        let file = self.find(id)?;
        if !file.is_folder() {
            return Ok(HashSet::default());
        }

        // Populate cache
        self.all_children()?;

        // Return value from cache
        if let Some(children) = self.children.get(id) {
            return Ok(children.clone());
        }

        Ok(HashSet::new())
    }

    // todo: cache?
    pub fn children_using_links(&mut self, id: &Uuid) -> SharedResult<HashSet<Uuid>> {
        let id = match self.find(id)?.file_type() {
            FileType::Document | FileType::Folder => *id,
            FileType::Link { target } => target,
        };
        self.children(&id)
    }

    /// Returns ids of files for which the argument is an ancestor—the files' children, recursively. Does not include the argument.
    /// This function tolerates cycles.
    pub fn descendants(&mut self, id: &Uuid) -> SharedResult<HashSet<Uuid>> {
        // todo: caching?
        let mut result = HashSet::new();
        let mut to_process = vec![*id];
        let mut i = 0;
        while i < to_process.len() {
            let new_descendents = self
                .children(&to_process[i])?
                .into_iter()
                .filter(|f| !result.contains(f))
                .collect::<Vec<Uuid>>();
            // TODO could consider optimizing by not exploring documents
            to_process.extend(new_descendents.iter());
            result.extend(new_descendents.into_iter());
            i += 1;
        }
        Ok(result)
    }

    pub fn descendants_using_links(&mut self, id: &Uuid) -> SharedResult<HashSet<Uuid>> {
        // todo: caching?
        let mut result = HashSet::new();
        let mut to_process = vec![*id];
        let mut i = 0;
        while i < to_process.len() {
            let new_descendents = self
                .children_using_links(&to_process[i])?
                .into_iter()
                .filter(|f| !result.contains(f))
                .collect::<Vec<Uuid>>();
            // TODO could consider optimizing by not exploring documents
            to_process.extend(new_descendents.iter());
            result.extend(new_descendents.into_iter());
            i += 1;
        }
        Ok(result)
    }

    // todo: move to TreeLike
    /// Returns ids of files for which the argument is a descendent—the files' parent, recursively. Does not include the argument.
    /// This function tolerates cycles.
    pub fn ancestors(&self, id: &Uuid) -> SharedResult<HashSet<Uuid>> {
        let mut result = HashSet::new();
        let mut current_file = self.find(id)?;
        while !current_file.is_root()
            && self.maybe_find(current_file.parent()).is_some()
            && !result.contains(current_file.parent())
        {
            result.insert(*current_file.parent());
            current_file = self.find_parent(current_file)?;
        }
        Ok(result)
    }

    pub fn decrypt_document(
        &mut self, id: &Uuid, encrypted: &EncryptedDocument, account: &Account,
    ) -> SharedResult<DecryptedDocument> {
        let key = self.decrypt_key(id, account)?;
        let compressed = symkey::decrypt(&key, encrypted)?;
        compression_service::decompress(&compressed)
    }

    pub fn stage<T2: TreeLikeMut<F = T::F>>(self, staged: T2) -> LazyTree<StagedTree<T, T2>> {
        // todo: optimize by performing minimal updates on self caches
        LazyTree::<StagedTree<T, T2>> {
            tree: StagedTree::new(self.tree, staged),
            name: HashMap::new(),
            key: self.key,
            implicit_deleted: HashMap::new(),
            children: HashMap::new(),
        }
    }

    pub fn stage_removals(self, removed: HashSet<Uuid>) -> LazyTree<StagedTree<T, Option<T::F>>> {
        // todo: optimize by performing minimal updates on self caches
        LazyTree::<StagedTree<T, Option<T::F>>> {
            tree: StagedTree::removal(self.tree, removed),
            name: HashMap::new(),
            key: self.key,
            implicit_deleted: HashMap::new(),
            children: HashMap::new(),
        }
    }

    // todo: optimize
    pub fn assert_names_decryptable(&mut self, account: &Account) -> SharedResult<()> {
        for id in self.owned_ids() {
            if self.name(&id, account).is_err() {
                return Err(SharedErrorKind::ValidationFailure(
                    ValidationFailure::NonDecryptableFileName(id),
                )
                .into());
            }
        }
        Ok(())
    }
}

pub type Stage1<Base, Local> = StagedTree<Base, Local>;
pub type LazyStaged1<Base, Local> = LazyTree<Stage1<Base, Local>>;
pub type Stage2<Base, Local, Staged> = StagedTree<StagedTree<Base, Local>, Staged>;
pub type LazyStage2<Base, Local, Staged> = LazyTree<Stage2<Base, Local, Staged>>;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Eq)]
pub enum ValidationFailure {
    Orphan(Uuid),
    Cycle(HashSet<Uuid>),
    PathConflict(HashSet<Uuid>),
    FileNameTooLong(Uuid),
    NonFolderWithChildren(Uuid),
    FileWithDifferentOwnerParent(Uuid),
    NonDecryptableFileName(Uuid),
    SharedLink { link: Uuid, shared_ancestor: Uuid },
    DuplicateLink { target: Uuid },
    BrokenLink(Uuid),
    OwnedLink(Uuid),
}

impl<T> LazyTree<T>
where
    T: TreeLikeMut,
{
    pub fn stage_and_promote<S: TreeLikeMut<F = T::F>>(
        &mut self, mut staged: S,
    ) -> SharedResult<()> {
        for id in staged.owned_ids() {
            if let Some(removed) = staged.remove(id)? {
                self.tree.insert(removed)?;
            }
        }
        // todo: incremental cache update
        self.name = HashMap::new();
        self.implicit_deleted = HashMap::new();
        self.children = HashMap::new();
        Ok(())
    }

    pub fn stage_validate_and_promote<S: TreeLikeMut<F = T::F>>(
        &mut self, mut staged: S, owner: Owner,
    ) -> SharedResult<()> {
        StagedTree::new(&self.tree, &mut staged)
            .to_lazy()
            .validate(owner)?;
        self.stage_and_promote(staged)?;
        Ok(())
    }

    pub fn stage_removals_and_promote(&mut self, removed: HashSet<Uuid>) -> SharedResult<()> {
        for id in removed {
            self.tree.remove(id)?;
        }
        // todo: incremental cache update
        self.name = HashMap::new();
        self.implicit_deleted = HashMap::new();
        self.children = HashMap::new();
        Ok(())
    }
}

impl<Base, Staged> LazyStaged1<Base, Staged>
where
    Base: TreeLikeMut,
    Staged: TreeLikeMut<F = Base::F>,
{
    // todo: incrementalism
    pub fn promote(self) -> SharedResult<LazyTree<Base>> {
        let mut staged = self.tree.staged;
        let mut base = self.tree.base;
        for id in staged.owned_ids() {
            if let Some(removed) = staged.remove(id)? {
                base.insert(removed)?;
            }
        }
        for id in self.tree.removed {
            base.remove(id)?;
        }

        Ok(LazyTree {
            tree: base,
            name: HashMap::new(),
            key: self.key,
            implicit_deleted: HashMap::new(),
            children: HashMap::new(),
        })
    }
}

impl<Base, Staged> LazyStaged1<Base, Staged>
where
    Base: TreeLike,
    Staged: TreeLikeMut<F = Base::F>,
{
    // todo: incrementalism
    pub fn unstage(self) -> (LazyTree<Base>, Staged) {
        (
            LazyTree {
                tree: self.tree.base,
                name: HashMap::new(),
                key: self.key,
                implicit_deleted: HashMap::new(),
                children: HashMap::new(),
            },
            self.tree.staged,
        )
    }
}

impl<T: TreeLike> TreeLike for LazyTree<T> {
    type F = T::F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.tree.ids()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        self.tree.maybe_find(id)
    }
}
