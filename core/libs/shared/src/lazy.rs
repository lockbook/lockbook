use crate::account::Account;
use crate::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::file::File;
use crate::file_like::FileLike;
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
        Ok(self
            .all_files()?
            .into_iter()
            .filter(|f| f.parent() == id && !f.is_root())
            .map(|f| *f.id())
            .collect())
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
            to_process.extend(new_descendents.iter());
            result.extend(new_descendents.into_iter());
            i += 1;
        }
        Ok(result)
    }

    /// Returns ids of files for which the argument is a descendent—the files' parent, recursively. Does not include the argument.
    /// This function tolerates cycles.
    pub fn ancestors(&mut self, id: &Uuid) -> SharedResult<HashSet<Uuid>> {
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

impl<Base: Stagable, Local: Stagable<F = Base::F>> LazyTree<StagedTree<Base, Local>> {
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

    pub fn validate(&mut self) -> SharedResult<()> {
        self.assert_no_orphans()?;
        // self.assert_no_cycles()?;
        Ok(())
    }

    pub fn resolve_merge_conflicts(&mut self) -> SharedResult<()> {
        self.prune_orphans()?;
        // self.unmove_moved_files_in_cycles()?;
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

    // assumptions: none
    // changes: prunes files
    // invalidated by: nothing
    fn prune_orphans(&mut self) -> SharedResult<()> {
        let mut to_prune = HashSet::new();
        for id in self.owned_ids() {
            if self.maybe_find_parent(self.find(&id)?).is_none() {
                to_prune.extend(self.descendents(&id)?);
            }
        }
        for id in to_prune {
            self.remove(id);
            self.name_by_id.remove(&id);
            self.key_by_id.remove(&id);
            self.implicitly_deleted_by_id.remove(&id);
        }
        Ok(())
    }

    //  // assumptions: no orphans
    // fn assert_no_cycles(&mut self) -> SharedResult<()> {
    //     let mut root_found = false;
    //     let mut prev_checked = HashSet::<Uuid>::new();
    //     for id in self.owned_ids() {
    //         let mut checking = HashSet::<Uuid>::new();
    //         let mut cur = self.find(&id)?;
    //         loop {
    //             match (cur.is_root(), root_found, prev_checked.contains(&cur.id())) {
    //                 (true, false, _) => root_found = true,
    //                 (true, true, _) => return Err(SharedError::ValidationFailure(ValidationFailure::Cycle(checking.into_iter().collect()))),
    //                 (false, _, false) => todo!(),
    //                 (false, _, true) => return Err(SharedError::ValidationFailure(ValidationFailure::Cycle(checking.into_iter().collect()))),
    //             }
    //         }

    //         while !cur.is_root() && prev_checked.get(&cur.id()).is_none() {
    //             if checking.contains_key(&cur.id()) {
    //                 result.extend(checking.keys());
    //                 break;
    //             }
    //             checking.push(cur.clone());
    //             if cur.is_shared_with_user(&user) {
    //                 break;
    //             }
    //             cur = &staged_changes.get(&cur.parent()).ok_or(FileNonexistent)?.0;
    //         }
    //         prev_checked.extend(checking);
    //     }
    //     Ok(())
    // }

    // // assumptions: no orphans
    // // changes: moves files
    // // invalidated by: moved files
    // fn unmove_moved_files_in_cycles(&mut self) -> SharedResult<()> {
    //     let mut root_found = false;
    //     let mut prev_checked = HashMap::new();
    //     let mut result = Vec::new();
    //     for id in self.owned_ids() {
    //         let mut checking = HashMap::new();
    //         let mut cur = self.find(&id)?;
    //         if cur.is_root() {
    //             if root_found {
    //                 result.push(cur.id());
    //             } else {
    //                 root_found = true;
    //             }
    //         }
    //         while !cur.is_root() && prev_checked.get(&cur.id()).is_none() {
    //             if checking.contains_key(&cur.id()) {
    //                 result.extend(checking.keys());
    //                 break;
    //             }
    //             checking.push(cur.clone());
    //             if cur.is_shared_with_user(&user) {
    //                 break;
    //             }
    //             cur = &staged_changes.get(&cur.parent()).ok_or(FileNonexistent)?.0;
    //         }
    //         prev_checked.extend(checking);
    //     }
    //     Ok(result)
    // }

    // fn get_path_conflicts(&mut self) -> SharedResult<()> {
    //     let files = self
    //         .clone()
    //         .stage(staged_changes.clone())
    //         .filter_not_deleted()?;
    //     let files_with_sources = self.stage_with_source(staged_changes);
    //     let mut name_tree: HashMap<Uuid, HashMap<Fm::Name, Uuid>> = HashMap::new();
    //     let mut result = Vec::new();
    //     for (id, f) in files {
    //         let source = files_with_sources
    //             .get(&id)
    //             .ok_or_else(|| {
    //                 TreeError::Unexpected(
    //                     "get_path_conflicts: could not find source by id".to_string(),
    //                 )
    //             })?
    //             .clone()
    //             .1;
    //         let parent_id = f.parent();
    //         if f.is_root() {
    //             continue;
    //         };
    //         let name = f.name();
    //         let parent_children = name_tree.entry(parent_id).or_insert_with(HashMap::new);
    //         let cloned_id = id;
    //         if let Some(conflicting_child_id) = parent_children.get(&name) {
    //             match source {
    //                 StageSource::Base => result.push(PathConflict {
    //                     existing: cloned_id,
    //                     staged: conflicting_child_id.to_owned(),
    //                 }),
    //                 StageSource::Staged => result.push(PathConflict {
    //                     existing: conflicting_child_id.to_owned(),
    //                     staged: cloned_id,
    //                 }),
    //             }
    //         } else {
    //             parent_children.insert(name, cloned_id);
    //         };
    //     }
    //     Ok(result)
    // }
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
