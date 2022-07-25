use crate::account::Account;
use crate::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::file::File;
use crate::file_like::FileLike;
use crate::file_metadata::FileMetadata;
use crate::filename::NameComponents;
use crate::secret_filename::SecretFileName;
use crate::staged::StagedTree;
use crate::tree_like::{Stagable, TreeLike};
use crate::{pubkey, symkey, SharedError, SharedResult};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub struct LazyTree<T: Stagable> {
    tree: T,
    name_by_id: HashMap<Uuid, String>,
    key_by_id: HashMap<Uuid, AESKey>,
    implicitly_deleted_by_id: HashMap<Uuid, bool>,
}

impl<T: Stagable> LazyTree<T> {
    pub fn new(tree: T) -> Self {
        Self {
            name_by_id: HashMap::new(),
            key_by_id: HashMap::new(),
            implicitly_deleted_by_id: HashMap::new(),
            tree,
        }
    }
}

impl<T: Stagable> LazyTree<T> {
    pub fn calculate_deleted(&mut self, id: &Uuid) -> SharedResult<bool> {
        let (visited_ids, deleted) = {
            let mut file = self.find(id)?;
            let mut visited_ids = vec![];
            let mut deleted = false;

            while !file.is_root() {
                visited_ids.push(*file.id());
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

    pub fn decrypt_key(&mut self, id: &Uuid, account: &Account) -> SharedResult<AESKey> {
        let mut file_id = *self.find(id)?.id();
        let mut visited_ids = vec![];

        loop {
            if self.key_by_id.get(&file_id).is_some() {
                break;
            }

            let maybe_file_key = if let Some(user_access) = self
                .find(&file_id)?
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
            file_id = *self.find_parent(&self.find(&file_id)?)?.id();
        }

        for id in visited_ids.iter().rev() {
            let decrypted_key = {
                let file = self.find(id)?;
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

    pub fn name(&mut self, id: &Uuid, account: &Account) -> SharedResult<String> {
        if let Some(name) = self.name_by_id.get(&id) {
            return Ok(name.clone());
        }

        let parent_id = *self.find(id)?.parent();
        let parent_key = self.decrypt_key(&parent_id, account)?;

        let name = self.find(id)?.secret_name().to_string(&parent_key)?;
        self.name_by_id.insert(*id, name.clone());
        Ok(name)
    }

    pub fn all_files(&mut self) -> SharedResult<Vec<&T::F>> {
        todo!()
    }

    /// Returns ids of files whose parent is the argument. Does not include the argument.
    pub fn children(&mut self, id: &Uuid) -> SharedResult<HashSet<Uuid>> {
        // todo: caching?
        Ok(self.all_files()?.into_iter().filter(|f| f.parent() == id && f.id() != id).map(|f| *f.id()).collect())
    }

    /// Returns ids of files for which the argument is an ancestor—the files' children, recursively. Does not include the argument.
    /// This function tolerates cycles.
    pub fn descendents(&mut self, id: &Uuid) -> SharedResult<HashSet<Uuid>> {
        // todo: caching?
        let mut result = HashSet::new();
        let mut to_process = vec![*id];
        let mut i = 0;
        while i < to_process.len() {
            let new_descendents = self.children(&to_process[i])?.into_iter().filter(|f| !result.contains(f)).collect::<Vec<Uuid>>();
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
        while !current_file.is_root() && !result.contains(current_file.parent()) {
            result.insert(*current_file.parent());
            current_file = self.find_parent(current_file)?;
        }
        Ok(result)
    }

    pub fn encrypt_document(
        &mut self, id: &Uuid, document: &DecryptedDocument, account: &Account,
    ) -> SharedResult<EncryptedDocument> {
        let key = self.decrypt_key(id, account)?;
        symkey::encrypt(&key, document)
    }

    pub fn decrypt_document(
        &mut self, id: &Uuid, encrypted: &EncryptedDocument, account: &Account,
    ) -> SharedResult<DecryptedDocument> {
        let key = self.decrypt_key(id, account)?;
        symkey::decrypt(&key, encrypted)
    }

    pub fn finalize(&mut self, id: &Uuid, account: &Account) -> SharedResult<File> {
        let meta = self.find(id)?;
        let file_type = meta.file_type();
        let parent = *meta.parent();
        let name = self.name(id, account)?;
        let id = *id;
        Ok(File { id, parent, name, file_type })
    }

    pub fn stage<T2: Stagable<F = T::F>>(self, staged: T2) -> LazyTree<StagedTree<T, T2>> {
        // todo: optimize by performing minimal updates on self caches
        LazyTree::<StagedTree<T, T2>> {
            name_by_id: HashMap::new(),
            key_by_id: self.key_by_id,
            implicitly_deleted_by_id: HashMap::new(),
            tree: StagedTree::<T, T2> { base: self.tree, staged },
        }
    }

    pub fn validate(&mut self) -> SharedResult<()> {
        self.assert_no_orphans()?;
        self.assert_no_cycles()?;
        self.assert_no_path_conflicts()?;
        Ok(())
    }

    fn assert_no_orphans(&mut self) -> SharedResult<()> {
        for file in self.ids().into_iter().filter_map(|id| self.maybe_find(id)) {
            if self.maybe_find_parent(file).is_none() {
                return Err(SharedError::ValidationFailure(ValidationFailure::Orphan(*file.id())));
            }
        }
        Ok(())
    }

    // assumption: no orphans
    fn assert_no_cycles(&mut self) -> SharedResult<()> {
       let mut root_found = false;
       let mut no_cycles_in_ancestors = HashSet::new();
       for id in self.owned_ids() {
           let mut ancestors = HashSet::new();
           let mut current_file = self.find(&id)?;
           loop {
               if no_cycles_in_ancestors.contains(&id) {
                   break;
               } else if current_file.is_root() {
                   if !root_found {
                       root_found = true;
                       break;
                   } else {
                       return Err(SharedError::ValidationFailure(ValidationFailure::Cycle(HashSet::from([id]))))
                   }
               } else if ancestors.contains(current_file.parent()) {
                   return Err(SharedError::ValidationFailure(ValidationFailure::Cycle(self.ancestors(current_file.id())?)))
               } 
               ancestors.insert(*current_file.id());
               current_file = self.find_parent(current_file)?;
           }
           no_cycles_in_ancestors.extend(ancestors);
       }
       Ok(())
    }

    // todo: optimize
   fn assert_no_path_conflicts(&mut self) -> SharedResult<()> {
        let mut children_by_parent_and_name = HashMap::<(Uuid, SecretFileName), HashSet<Uuid>>::new();
        for id in self.owned_ids() {
            if !self.calculate_deleted(&id)? {
                let file = self.find(&id)?;
                children_by_parent_and_name.entry((*file.parent(), file.secret_name().clone())).or_insert_with(HashSet::new).insert(*file.id());
            }
        }
        for (_, siblings_with_same_name) in children_by_parent_and_name {
            if siblings_with_same_name.len() > 1 {
                return Err(SharedError::ValidationFailure(ValidationFailure::PathConflict(siblings_with_same_name)))
            }
        }
        Ok(())
   }
}

#[derive(Debug, PartialEq)]
pub enum ValidationFailure {
    Orphan(Uuid),
    Cycle(HashSet<Uuid>),
    PathConflict(HashSet<Uuid>),
    SharedLink(Uuid),
    DuplicateLink(HashSet<Uuid>),
    BrokenLink(Uuid),
}

impl<Base: Stagable<F = FileMetadata>, Local: Stagable<F = Base::F>> LazyTree<StagedTree<Base, Local>> {
    pub fn get_changes(&self) -> SharedResult<Vec<&Base::F>> {
        let base = self.tree.base.ids();
        let local = self.tree.staged.ids();
        let exists_both = local.iter().filter(|id| base.contains(**id));

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

    // todo: optimize subroutines by checking only staged things
    pub fn resolve_merge_conflicts(mut self, account: &Account) -> SharedResult<Self> {
        let mut change = self.unmove_moved_files_in_cycles()?;
        self = self.stage(change).promote();
        change = self.rename_files_with_path_conflicts(account)?;
        self = self.stage(change).promote();
        Ok(self)
    }

    // assumptions: no orphans
    // changes: moves files
    // invalidated by: moved files
    fn unmove_moved_files_in_cycles(&mut self) -> SharedResult<Vec<Base::F>> {
        let mut root_found = false;
        let mut no_cycles_in_ancestors = HashSet::new();
        let mut to_revert = HashSet::new();
        for id in self.owned_ids() {
            let mut ancestors = HashSet::new();
            let mut current_file = self.find(&id)?;
            loop {
                if no_cycles_in_ancestors.contains(&id) {
                    break;
                } else if current_file.is_root() {
                    if !root_found {
                        root_found = true;
                        break;
                    } else {
                        to_revert.insert(id);
                        break;
                    }
                } else if ancestors.contains(current_file.parent()) {
                    to_revert.extend(self.ancestors(current_file.id())?);
                    break;
                } 
                ancestors.insert(*current_file.id());
                current_file = self.find_parent(current_file)?;
            }
            no_cycles_in_ancestors.extend(ancestors);
        }
        let mut result = Vec::new();
        for id in to_revert {
            match (self.tree.base.maybe_find(&id), self.tree.staged.maybe_find(&id)) {
                (Some(base), Some(staged)) => {
                    let mut update = staged.clone();
                    update.parent = base.parent;
                    result.push(update);
                },
                _ => {},
            }
        }
        Ok(result)
    }

    // assumptions: no orphans
    // changes: renames files
    // invalidated by: moved files, renamed files
    fn rename_files_with_path_conflicts(&mut self, account: &Account) -> SharedResult<Vec<Base::F>> {
        let mut children = HashMap::<Uuid, HashSet<Uuid>>::new();
        let mut names = HashMap::<Uuid, String>::new();
        let mut keys = HashMap::<Uuid, AESKey>::new();
        for id in self.owned_ids() {
            if !self.calculate_deleted(&id)? {
                let file = self.find(&id)?;
                children.entry(*file.parent()).or_insert_with(HashSet::new).insert(id);
                names.insert(id, self.name(&id, account)?);
                keys.insert(id, self.decrypt_key(&id, account)?);
            }
        }

        let mut result = Vec::new();
        for (_, sibling_ids) in children {
            let siblings = sibling_ids.iter().filter_map(|s| self.maybe_find(&s)).collect::<Vec<_>>();
            for sibling in siblings {
                let mut name = names.get(sibling.id()).ok_or(SharedError::FileNonexistent)?.clone();
                let mut changed = false;
                while sibling_ids.iter().filter_map(|id| names.get(id)).any(|sibling_name| sibling_name == &name) {
                    name = NameComponents::from(&name).generate_next().to_name();
                    changed = true;
                }
                if changed {
                    let mut update = sibling.clone();
                    update.name = SecretFileName::from_str(&name, &keys[sibling.parent()])?;
                    result.push(update);
                }
            }
        }
        Ok(result)
    }
}

pub type Stage1<Base, Local> = StagedTree<Base, Local>;
pub type LazyStaged1<Base, Local> = LazyTree<Stage1<Base, Local>>;
pub type Stage2<Base, Local, Staged> = StagedTree<StagedTree<Base, Local>, Staged>;
pub type LazyStage2<Base, Local, Staged> = LazyTree<Stage2<Base, Local, Staged>>;

impl<Base, Local, Staged> LazyStage2<Base, Local, Staged>
where
    Base: Stagable,
    Local: Stagable<F = Base::F>,
    Staged: Stagable<F = Base::F>,
{
    pub fn promote(self) -> LazyStaged1<Base, Local> {
        let mut staged = self.tree.staged;
        let mut base = self.tree.base;
        for id in staged.owned_ids() {
            if let Some(removed) = staged.remove(id) {
                base.insert(removed);
            }
        }

        // todo: optimize by performing minimal updates on self caches
        LazyStaged1 {
            tree: base,
            name_by_id: HashMap::new(),
            key_by_id: self.key_by_id,
            implicitly_deleted_by_id: HashMap::new(),
        }
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
