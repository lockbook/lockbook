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

pub enum ValidationFailure {
    Orphan(Uuid),
    SelfDescendent(Vec<Uuid>),
    PathConflict(Vec<Uuid>),
    SharedLink(Uuid),
    DuplicateLink(Vec<Uuid>),
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

    // todo: remove?
    pub fn validate(&mut self) -> SharedResult<()> {
        todo!()
    }

    pub fn resolve_validation_failures(&mut self) -> SharedResult<()> {
        while let Some(validation_failure) = self.get_validation_failures()? {
            match validation_failure {
                ValidationFailure::Orphan(id) => {
                    self.remove(id);
                    //todo: minimally invalidate cache
                    self.name_by_id.remove(&id);
                    self.key_by_id.remove(&id);
                    self.implicitly_deleted_by_id.remove(&id);
                }
                ValidationFailure::SelfDescendent(_) => todo!(),
                ValidationFailure::PathConflict(_) => todo!(),
                ValidationFailure::SharedLink(_) => todo!(),
                ValidationFailure::DuplicateLink(_) => todo!(),
                ValidationFailure::BrokenLink(_) => todo!(),
            }
        }
        Ok(())
    }

    pub fn get_validation_failures(&mut self) -> SharedResult<Option<ValidationFailure>> {
        todo!()
        // self.get_orphans()?;
        // self.get_invalid_cycles()?;
        // self.get_path_conflicts()?;
        // self.get_shared_links()?;
        // self.get_duplicate_links()?;
        // self.get_broken_links()
    }

    fn get_orphans(&mut self) -> SharedResult<Option<Uuid>> {
        Ok(None)
    }

    // fn get_self_descendents(&mut self) -> SharedResult<Option<Vec<Uuid>>> {
    //     let mut root_found = false;
    //     let mut prev_checked = HashMap::new();
    //     let mut result = Vec::new();
    //     for id in self.ids() {

    //     }

    //     let mut root_found = false;
    //     let mut prev_checked = HashMap::new();
    //     let staged_changes = self.stage_with_source(staged_changes);
    //     let mut result = Vec::new();
    //     for (_, (f, _)) in staged_changes.clone().into_iter() {
    //         let mut checking = HashMap::new();
    //         let mut cur = &f;
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

    // fn get_shared_links(&mut self) -> SharedResult<()> {
    //     let mut result = Vec::new();
    //     for root in self
    //         .values()
    //         .filter(|f| f.is_root() || f.is_shared_with_user(user))
    //     {
    //         get_shared_links_helper(
    //             &self.stage_with_source(staged_changes),
    //             &root,
    //             &mut Vec::new(),
    //             &mut result,
    //         )?;
    //     }

    //     Ok(result)
    // }

    // fn get_duplicate_links(&mut self) -> SharedResult<()> {
    //     let mut links_by_target = HashMap::<Uuid, Vec<(Uuid, StageSource)>>::new();
    //     for file in self.stage_with_source(staged_changes).values() {
    //         if let FileType::Link { linked_file } = file.0.file_type() {
    //             match links_by_target.get_mut(&linked_file) {
    //                 Some(links) => links.push((file.0.id(), file.1)),
    //                 None => {
    //                     links_by_target.insert(linked_file, vec![(file.0.id(), file.1)]);
    //                 }
    //             }
    //         }
    //     }

    //     Ok(links_by_target
    //         .into_iter()
    //         .filter_map(
    //             |(target, links)| {
    //                 if links.len() > 1 {
    //                     Some(LinkDuplicate { links, target })
    //                 } else {
    //                     None
    //                 }
    //             },
    //         )
    //         .collect())
    // }

    // fn get_broken_links(&mut self) -> SharedResult<()> {
    //     let mut result = Vec::new();
    //     let files = self.clone().stage(staged_changes.clone()); // todo(sharing): don't clone
    //     let not_deleted_files = files.clone().filter_not_deleted()?; // todo(sharing): don't clone
    //     for file in files.values() {
    //         if let FileType::Link { linked_file } = file.file_type() {
    //             if not_deleted_files.maybe_find_ref(linked_file).is_none() {
    //                 result.push(file.id());
    //             }
    //         }
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
