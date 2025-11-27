use crate::model::access_info::UserAccessMode;
use crate::model::crypto::AESKey;
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{FileType, Owner};
use crate::model::staged::StagedTree;
use crate::model::symkey;
use crate::model::tree_like::{TreeLike, TreeLikeMut};
use crate::service::keychain::Keychain;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct LazyTree<T: TreeLike> {
    pub tree: T,
    pub name: HashMap<Uuid, String>,
    pub implicit_deleted: HashMap<Uuid, bool>,
    pub linked_by: HashMap<Uuid, Uuid>,
    pub children: HashMap<Uuid, HashSet<Uuid>>,
}

impl<T: TreeLike> LazyTree<T> {
    pub fn new(tree: T) -> Self {
        Self {
            name: HashMap::new(),
            implicit_deleted: HashMap::new(),
            children: HashMap::new(),
            linked_by: HashMap::new(),
            tree,
        }
    }
}

impl<T: TreeLike> LazyTree<T> {
    pub fn access_mode(&self, owner: Owner, id: &Uuid) -> LbResult<Option<UserAccessMode>> {
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

    pub fn in_pending_share(&mut self, id: &Uuid) -> LbResult<bool> {
        let mut id = *id;
        loop {
            if self.find(&id)?.parent() == self.find(&id)?.id() {
                return Ok(false); // root
            } else if let Some(link) = self.linked_by(&id)? {
                id = link;
            } else if self.maybe_find(self.find(&id)?.parent()).is_some() {
                id = *self.find(&id)?.parent();
            } else {
                return Ok(true); // share root
            }
        }
    }

    pub fn all_children(&mut self) -> LbResult<&HashMap<Uuid, HashSet<Uuid>>> {
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

    pub fn calculate_deleted(&mut self, id: &Uuid) -> LbResult<bool> {
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
                            return Err(LbErrKind::FileParentNonexistent)?;
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

    pub fn decrypt_key(&mut self, id: &Uuid, keychain: &Keychain) -> LbResult<AESKey> {
        let mut file_id = *self.find(id)?.id();
        let mut visited_ids = vec![];

        loop {
            if keychain.contains_aes_key(&file_id)? {
                break;
            }

            let my_pk = keychain.get_pk()?;

            let maybe_file_key = if let Some(user_access) = self
                .find(&file_id)?
                .user_access_keys()
                .iter()
                .find(|access| access.encrypted_for == my_pk)
            {
                Some(user_access.decrypt(keychain.get_account()?)?)
            } else {
                None
            };
            if let Some(file_key) = maybe_file_key {
                keychain.insert_aes_key(file_id, file_key)?;
                break;
            }

            visited_ids.push(file_id);
            file_id = *self.find_parent(self.find(&file_id)?)?.id();
        }

        for id in visited_ids.iter().rev() {
            let decrypted_key = {
                let file = self.find(id)?;
                let parent = self.find_parent(file)?;
                let parent_key =
                    keychain
                        .get_aes_key(parent.id())?
                        .ok_or(LbErrKind::Unexpected(
                            "parent key should have been populated by prior routine".to_string(),
                        ))?;
                let encrypted_key = file.folder_access_key();
                symkey::decrypt(&parent_key, encrypted_key)?
            };
            keychain.insert_aes_key(*id, decrypted_key)?;
        }

        Ok(keychain.get_aes_key(id)?.ok_or(LbErrKind::Unexpected(
            "parent key should have been populated by prior routine (2)".to_string(),
        ))?)
    }

    pub fn name(&mut self, id: &Uuid, keychain: &Keychain) -> LbResult<String> {
        if let Some(name) = self.name.get(id) {
            return Ok(name.clone());
        }
        let key = self.decrypt_key(id, keychain)?;
        let name = self.find(id)?.secret_name().to_string(&key)?;
        self.name.insert(*id, name.clone());
        Ok(name)
    }

    pub fn name_using_links(&mut self, id: &Uuid, keychain: &Keychain) -> LbResult<String> {
        let id = if let Some(link) = self.linked_by(id)? { link } else { *id };
        self.name(&id, keychain)
    }

    pub fn parent_using_links(&mut self, id: &Uuid) -> LbResult<Uuid> {
        let id = if let Some(link) = self.linked_by(id)? { link } else { *id };
        Ok(*self.find(&id)?.parent())
    }

    pub fn linked_by(&mut self, id: &Uuid) -> LbResult<Option<Uuid>> {
        if self.linked_by.is_empty() {
            for link_id in self.ids() {
                if let FileType::Link { target } = self.find(&link_id)?.file_type() {
                    if !self.calculate_deleted(&link_id)? {
                        self.linked_by.insert(target, link_id);
                    }
                }
            }
        }

        Ok(self.linked_by.get(id).copied())
    }

    /// Returns ids of files whose parent is the argument. Does not include the argument.
    /// TODO could consider returning a reference to the underlying cached value
    pub fn children(&mut self, id: &Uuid) -> LbResult<HashSet<Uuid>> {
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
    pub fn children_using_links(&mut self, id: &Uuid) -> LbResult<HashSet<Uuid>> {
        let mut children = HashSet::new();
        for child in self.children(id)? {
            if let FileType::Link { target } = self.find(&child)?.file_type() {
                if !self.calculate_deleted(&child)? {
                    children.insert(target);
                }
            } else {
                children.insert(child);
            }
        }

        Ok(children)
    }

    /// Returns ids of files for which the argument is an ancestor—the files' children, recursively.
    /// Does not include the argument.
    /// This function tolerates cycles.
    pub fn descendants(&mut self, id: &Uuid) -> LbResult<HashSet<Uuid>> {
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

    pub fn descendants_using_links(&mut self, id: &Uuid) -> LbResult<HashSet<Uuid>> {
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
    pub fn ancestors(&self, id: &Uuid) -> LbResult<HashSet<Uuid>> {
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

    pub fn pending_roots(&mut self, keychain: &Keychain) -> LbResult<Vec<Uuid>> {
        let mut result = Vec::new();
        let owner = Owner(keychain.get_pk()?);
        for id in self.ids() {
            // file must be owned by another user
            if self.find(&id)?.owner() == owner {
                continue;
            }

            // file must be shared with this user
            if self.find(&id)?.access_mode(&owner).is_none() {
                continue;
            }

            // file must not be deleted
            if self.calculate_deleted(&id)? {
                continue;
            }

            // file must not have any links pointing to it
            if self.linked_by(&id)?.is_some() {
                continue;
            }

            result.push(id);
        }

        Ok(result)
    }

    pub fn non_deleted_pending_files(&mut self, keychain: &Keychain) -> LbResult<Vec<Uuid>> {
        let roots = self.pending_roots(keychain)?;
        let mut result = vec![];

        for id in roots {

            result.push(id);
            let descendants = self.descendants(&id)?;

            for id in descendants {
                if !self.calculate_deleted(&id)? {
                    result.push(id);
                }
            }
        }

        Ok(result)
    }

    pub fn stage<T2: TreeLikeMut<F = T::F>>(self, staged: T2) -> LazyTree<StagedTree<T, T2>> {
        // todo: optimize by performing minimal updates on self caches
        LazyTree::<StagedTree<T, T2>> {
            tree: StagedTree::new(self.tree, staged),
            name: HashMap::new(),
            implicit_deleted: HashMap::new(),
            children: HashMap::new(),
            linked_by: HashMap::new(),
        }
    }

    // todo: this is dead code
    pub fn stage_removals(self, removed: HashSet<Uuid>) -> LazyTree<StagedTree<T, Option<T::F>>> {
        // todo: optimize by performing minimal updates on self caches
        LazyTree::<StagedTree<T, Option<T::F>>> {
            tree: StagedTree::removal(self.tree, removed),
            name: HashMap::new(),
            implicit_deleted: HashMap::new(),
            children: HashMap::new(),
            linked_by: HashMap::new(),
        }
    }

    // todo: optimize
    pub fn assert_names_decryptable(&mut self, keychain: &Keychain) -> LbResult<()> {
        for id in self.ids() {
            if self.name(&id, keychain).is_err() {
                return Err(LbErrKind::Validation(ValidationFailure::NonDecryptableFileName(id)))?;
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

    /// A folder was moved into itself
    Cycle(HashSet<Uuid>),

    /// A filename is not available
    PathConflict(HashSet<Uuid>),

    /// This filename is too long
    FileNameTooLong(Uuid),

    /// A link or document was treated as a folder
    NonFolderWithChildren(Uuid),

    /// You cannot have a link to a file you own
    OwnedLink(Uuid),

    /// You cannot have a link that points to a nonexistent file
    BrokenLink(Uuid),

    /// You cannot have multiple links to the same file
    DuplicateLink {
        target: Uuid,
    },

    /// You cannot have a link inside a shared folder
    SharedLink {
        link: Uuid,
        shared_ancestor: Uuid,
    },

    FileWithDifferentOwnerParent(Uuid),
    NonDecryptableFileName(Uuid),
    DeletedFileUpdated(Uuid),
}

impl<T> LazyTree<T>
where
    T: TreeLikeMut,
{
    pub fn stage_and_promote<S: TreeLikeMut<F = T::F>>(&mut self, mut staged: S) -> LbResult<()> {
        for id in staged.ids() {
            if let Some(removed) = staged.remove(id)? {
                self.tree.insert(removed)?;
            }
        }
        // todo: incremental cache update
        self.name = HashMap::new();
        self.implicit_deleted = HashMap::new();
        self.children = HashMap::new();
        self.linked_by = HashMap::new();
        Ok(())
    }

    pub fn stage_validate_and_promote<S: TreeLikeMut<F = T::F>>(
        &mut self, mut staged: S, owner: Owner,
    ) -> LbResult<()> {
        StagedTree::new(&self.tree, &mut staged)
            .to_lazy()
            .validate(owner)?;
        self.stage_and_promote(staged)?;
        Ok(())
    }

    // todo: this is dead code
    pub fn stage_removals_and_promote(&mut self, removed: HashSet<Uuid>) -> LbResult<()> {
        for id in removed {
            self.tree.remove(id)?;
        }
        // todo: incremental cache update
        self.name = HashMap::new();
        self.implicit_deleted = HashMap::new();
        self.children = HashMap::new();
        self.linked_by = HashMap::new();
        Ok(())
    }
}

impl<Base, Staged> LazyStaged1<Base, Staged>
where
    Base: TreeLikeMut,
    Staged: TreeLikeMut<F = Base::F>,
{
    // todo: incrementalism
    pub fn promote(self) -> LbResult<LazyTree<Base>> {
        let mut staged = self.tree.staged;
        let mut base = self.tree.base;
        for id in staged.ids() {
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
            implicit_deleted: HashMap::new(),
            children: HashMap::new(),
            linked_by: HashMap::new(),
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
                implicit_deleted: HashMap::new(),
                children: HashMap::new(),
                linked_by: HashMap::new(),
            },
            self.tree.staged,
        )
    }
}

impl<T: TreeLike> TreeLike for LazyTree<T> {
    type F = T::F;

    fn ids(&self) -> Vec<Uuid> {
        self.tree.ids()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        self.tree.maybe_find(id)
    }
}
