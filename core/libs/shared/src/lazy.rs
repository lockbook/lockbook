use crate::access_info::UserAccessMode;
use crate::account::Account;
use crate::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::file_like::FileLike;
use crate::file_metadata::{FileType, Owner};
use crate::staged::StagedTree;
use crate::tree_like::{Stagable, TreeLike};
use crate::{compression_service, symkey, SharedError, SharedResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug)]
pub struct LazyTree<T: Stagable> {
    pub tree: T,
    pub name: HashMap<Uuid, String>,
    pub key: HashMap<Uuid, AESKey>,
    pub implicit_deleted: HashMap<Uuid, bool>,
    pub children: HashMap<Uuid, HashSet<Uuid>>,
}

impl<T: Stagable> LazyTree<T> {
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

impl<T: Stagable> LazyTree<T> {
    // todo: use greatest of all access modes instead of access mode of nearest keyed ancestor
    pub fn access_mode(&self, owner: Owner, id: &Uuid) -> SharedResult<Option<UserAccessMode>> {
        let mut file = self.find(id)?;
        loop {
            if let Some(access_mode) = file.access_mode(&owner) {
                return Ok(Some(access_mode));
            } else if file.parent() == file.id() {
                return Ok(None); // root
            } else if let Some(parent) = self.maybe_find(file.parent()) {
                file = parent
            } else {
                return Ok(None); // share root
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

            while !file.is_root() && !visited_ids.contains(file.parent()) {
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
                            return Err(SharedError::FileParentNonexistent);
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
                let parent_key = self.key.get(parent.id()).ok_or(SharedError::Unexpected(
                    "parent key should have been populated by prior routine",
                ))?;
                let encrypted_key = file.folder_access_key();
                symkey::decrypt(parent_key, encrypted_key)?
            };
            self.key.insert(*id, decrypted_key);
        }

        Ok(*self.key.get(id).ok_or(SharedError::Unexpected(
            "parent key should have been populated by prior routine (2)",
        ))?)
    }

    pub fn name(&mut self, id: &Uuid, account: &Account) -> SharedResult<String> {
        if let Some(name) = self.name.get(id) {
            return Ok(name.clone());
        }
        let id = if let Some(link) = self.link(id)? { link } else { *id };
        let key = self.decrypt_key(&id, account)?;
        let name = self.find(&id)?.secret_name().to_string(&key)?;
        self.name.insert(id, name.clone());
        Ok(name)
    }

    pub fn link(&self, id: &Uuid) -> SharedResult<Option<Uuid>> {
        for link_id in self.owned_ids() {
            if let FileType::Link { target } = self.find(&link_id)?.file_type() {
                if id == &target {
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

    /// Returns ids of files for which the argument is an ancestor—the files' children, recursively. Does not include the argument.
    /// This function tolerates cycles.
    pub fn descendents(&mut self, id: &Uuid) -> SharedResult<HashSet<Uuid>> {
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

    pub fn stage<T2: Stagable<F = T::F>>(self, staged: T2) -> LazyTree<StagedTree<T, T2>> {
        // todo: optimize by performing minimal updates on self caches
        LazyTree::<StagedTree<T, T2>> {
            name: HashMap::new(),
            key: self.key,
            implicit_deleted: HashMap::new(),
            tree: StagedTree::new(self.tree, staged),
            children: HashMap::new(),
        }
    }

    pub fn validate(&mut self) -> SharedResult<()> {
        self.assert_all_files_decryptable()?;
        self.assert_only_folders_have_children()?;
        self.assert_no_cycles()?;
        self.assert_no_path_conflicts()?;
        // todo
        // self.assert_names_decryptable(account)?;
        self.assert_no_shared_links()?;
        self.assert_no_duplicate_links()?;
        self.assert_no_broken_links()?;
        self.assert_no_owned_links()?;

        Ok(())
    }

    pub fn assert_only_folders_have_children(&self) -> SharedResult<()> {
        for file in self.all_files()? {
            if let Some(parent) = self.maybe_find(file.parent()) {
                if !parent.is_folder() {
                    return Err(SharedError::ValidationFailure(
                        ValidationFailure::NonFolderWithChildren(*parent.id()),
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn assert_all_files_decryptable(&mut self) -> SharedResult<()> {
        for file in self.ids().into_iter().filter_map(|id| self.maybe_find(id)) {
            // todo: user access key for this user
            if self.maybe_find_parent(file).is_none() && file.user_access_keys().is_empty() {
                return Err(SharedError::ValidationFailure(ValidationFailure::Orphan(*file.id())));
            }
        }
        Ok(())
    }

    // assumption: no orphans
    pub fn assert_no_cycles(&mut self) -> SharedResult<()> {
        let mut root_found = false;
        let mut no_cycles_in_ancestors = HashSet::new();
        for id in self.owned_ids() {
            let mut ancestors = HashSet::new();
            let mut current_file = self.find(&id)?;
            loop {
                if no_cycles_in_ancestors.contains(current_file.id()) {
                    break;
                } else if current_file.is_root() {
                    if !root_found {
                        root_found = true;
                        ancestors.insert(*current_file.id());
                        break;
                    } else {
                        return Err(SharedError::ValidationFailure(ValidationFailure::Cycle(
                            HashSet::from([id]),
                        )));
                    }
                } else if ancestors.contains(current_file.parent()) {
                    return Err(SharedError::ValidationFailure(ValidationFailure::Cycle(
                        self.ancestors(current_file.id())?,
                    )));
                }
                ancestors.insert(*current_file.id());
                current_file = match self.maybe_find_parent(current_file) {
                    Some(file) => file,
                    None => {
                        if !current_file.user_access_keys().is_empty() {
                            break;
                        } else {
                            return Err(SharedError::FileParentNonexistent);
                        }
                    }
                }
            }
            no_cycles_in_ancestors.extend(ancestors);
        }
        Ok(())
    }

    // todo: optimize
    pub fn assert_no_path_conflicts(&mut self) -> SharedResult<()> {
        let mut id_by_name = HashMap::new();
        for id in self.owned_ids() {
            if !self.calculate_deleted(&id)? {
                let file = self.find(&id)?;
                if file.is_root() {
                    continue;
                }
                if let Some(conflicting) = id_by_name.remove(file.secret_name()) {
                    return Err(SharedError::ValidationFailure(ValidationFailure::PathConflict(
                        HashSet::from([conflicting, *file.id()]),
                    )));
                }
                id_by_name.insert(file.secret_name().clone(), *file.id());
            }
        }
        Ok(())
    }

    // todo: optimize
    pub fn assert_names_decryptable(&mut self, account: &Account) -> SharedResult<()> {
        for id in self.owned_ids() {
            if self.name(&id, account).is_err() {
                return Err(SharedError::ValidationFailure(
                    ValidationFailure::NonDecryptableFileName(id),
                ));
            }
        }
        Ok(())
    }

    pub fn assert_no_shared_links(&self) -> SharedResult<()> {
        for link in self.owned_ids() {
            if let FileType::Link { target: _ } = self.find(&link)?.file_type() {
                for ancestor in self.ancestors(&link)? {
                    if self.find(&ancestor)?.is_shared() {
                        return Err(SharedError::ValidationFailure(
                            ValidationFailure::SharedLink { link, shared_ancestor: ancestor },
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn assert_no_duplicate_links(&self) -> SharedResult<()> {
        let mut linked_targets = HashSet::new();
        for link in self.owned_ids() {
            if let FileType::Link { target } = self.find(&link)?.file_type() {
                if !linked_targets.insert(target) {
                    return Err(SharedError::ValidationFailure(ValidationFailure::DuplicateLink {
                        target,
                    }));
                }
            }
        }
        Ok(())
    }

    pub fn assert_no_broken_links(&self) -> SharedResult<()> {
        for link in self.owned_ids() {
            if let FileType::Link { target } = self.find(&link)?.file_type() {
                if self.maybe_find(&target).is_none() {
                    return Err(SharedError::ValidationFailure(ValidationFailure::BrokenLink(
                        link,
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn assert_no_owned_links(&self) -> SharedResult<()> {
        for link in self.owned_ids() {
            if let FileType::Link { target } = self.find(&link)?.file_type() {
                if self.find(&link)?.owner() == self.find(&target)?.owner() {
                    return Err(SharedError::ValidationFailure(ValidationFailure::OwnedLink(link)));
                }
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
    NonFolderWithChildren(Uuid),
    NonDecryptableFileName(Uuid),
    SharedLink { link: Uuid, shared_ancestor: Uuid },
    DuplicateLink { target: Uuid },
    BrokenLink(Uuid),
    OwnedLink(Uuid),
}

impl<Base, Staged> LazyStaged1<Base, Staged>
where
    Base: Stagable,
    Staged: Stagable<F = Base::F>,
{
    // todo: incrementalism
    pub fn promote(self) -> LazyTree<Base> {
        let mut staged = self.tree.staged;
        let mut base = self.tree.base;
        for id in staged.owned_ids() {
            if let Some(removed) = staged.remove(id) {
                base.insert(removed);
            }
        }

        LazyTree {
            tree: base,
            name: HashMap::new(),
            key: HashMap::new(),
            implicit_deleted: HashMap::new(),
            children: HashMap::new(),
        }
    }

    // todo: incrementalism
    pub fn unstage(self) -> (LazyTree<Base>, Staged) {
        (
            LazyTree {
                tree: self.tree.base,
                name: HashMap::new(),
                key: HashMap::new(),
                implicit_deleted: HashMap::new(),
                children: HashMap::new(),
            },
            self.tree.staged,
        )
    }
}

impl<T: Stagable> TreeLike for LazyTree<T> {
    type F = T::F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.tree.ids()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        self.tree.maybe_find(id)
    }

    fn insert(&mut self, f: Self::F) -> Option<Self::F> {
        self.tree.insert(f)
    }

    fn remove(&mut self, id: Uuid) -> Option<Self::F> {
        self.tree.remove(id)
    }
}
