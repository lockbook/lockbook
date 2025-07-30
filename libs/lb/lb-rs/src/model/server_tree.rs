use crate::model::file_like::FileLike;
use crate::model::file_metadata::Owner;
use crate::model::tree_like::{TreeLike, TreeLikeMut};
use db_rs::{LookupSet, LookupTable};
use std::collections::HashSet;
use std::iter::FromIterator;
use tracing::*;
use uuid::Uuid;

use super::errors::LbResult;
use super::server_meta::ServerMeta;

pub struct ServerTree<'a> {
    pub ids: HashSet<Uuid>,
    pub owned_files: &'a mut LookupSet<Owner, Uuid>,
    pub shared_files: &'a mut LookupSet<Owner, Uuid>,
    pub file_children: &'a mut LookupSet<Uuid, Uuid>,
    pub files: &'a mut LookupTable<Uuid, ServerMeta>,
}

impl<'a> ServerTree<'a> {
    pub fn new(
        owner: Owner, owned_files: &'a mut LookupSet<Owner, Uuid>,
        shared_files: &'a mut LookupSet<Owner, Uuid>, file_children: &'a mut LookupSet<Uuid, Uuid>,
        files: &'a mut LookupTable<Uuid, ServerMeta>,
    ) -> LbResult<Self> {
        let (owned_ids, shared_ids) =
            match (owned_files.get().get(&owner), shared_files.get().get(&owner)) {
                (Some(owned_ids), Some(shared_ids)) => (owned_ids.clone(), shared_ids.clone()),
                _ => {
                    error!("Tree created for user without owned and shared files {:?}", owner);
                    (HashSet::new(), HashSet::new())
                }
            };

        let mut ids = HashSet::new();
        ids.extend(owned_ids);
        ids.extend(shared_ids.clone());

        let mut to_get_descendants = Vec::from_iter(shared_ids);
        while let Some(id) = to_get_descendants.pop() {
            let children = file_children.get().get(&id).cloned().unwrap_or_default();
            ids.extend(children.clone());
            to_get_descendants.extend(children);
        }

        Ok(Self { ids, owned_files, shared_files, file_children, files })
    }
}

impl TreeLike for ServerTree<'_> {
    type F = ServerMeta;

    fn ids(&self) -> Vec<Uuid> {
        self.ids.iter().copied().collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        if self.ids.contains(id) { self.files.maybe_find(id) } else { None }
    }
}

impl TreeLikeMut for ServerTree<'_> {
    fn insert(&mut self, f: Self::F) -> LbResult<Option<Self::F>> {
        let id = *f.id();
        let owner = f.owner();
        let maybe_prior = LookupTable::insert(self.files, id, f.clone())?;

        // maintain index: owned_files
        if maybe_prior.as_ref().map(|f| f.owner()) != Some(f.owner()) {
            if let Some(ref prior) = maybe_prior {
                self.owned_files.remove(&prior.owner(), &id)?;
            }
            self.owned_files.insert(owner, id)?;
        }

        // maintain index: shared_files
        let prior_sharees = if let Some(ref prior) = maybe_prior {
            prior
                .user_access_keys()
                .iter()
                .filter(|k| !k.deleted)
                .map(|k| Owner(k.encrypted_for))
                .collect()
        } else {
            HashSet::new()
        };
        let sharees = f
            .user_access_keys()
            .iter()
            .filter(|k| !k.deleted)
            .map(|k| Owner(k.encrypted_for))
            .collect::<HashSet<_>>();
        for removed_sharee in prior_sharees.difference(&sharees) {
            self.shared_files.remove(removed_sharee, &id)?;
        }
        for new_sharee in sharees.difference(&prior_sharees) {
            self.shared_files.insert(*new_sharee, id)?;
        }

        // maintain index: file_children
        if self.file_children.get().get(&id).is_none() {
            self.file_children.create_key(id)?;
        }
        if self.file_children.get().get(f.parent()).is_none() {
            self.file_children.create_key(*f.parent())?;
        }
        if maybe_prior.as_ref().map(|f| *f.parent()) != Some(*f.parent()) {
            if let Some(ref prior) = maybe_prior {
                self.file_children.remove(prior.parent(), &id)?;
            }

            self.file_children.insert(*f.parent(), id)?;
        }

        Ok(maybe_prior)
    }

    fn remove(&mut self, _id: Uuid) -> LbResult<Option<Self::F>> {
        error!("remove metadata called in server!");
        Ok(None)
    }

    fn clear(&mut self) -> LbResult<()> {
        error!("clear called in server!");
        Ok(())
    }
}
