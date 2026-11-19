use crate::model::file_like::FileLike;
use crate::model::file_metadata::Owner;
use crate::model::tree_like::{TreeLike, TreeLikeMut};
use db_rs::views::{hashmap::DbHashMap, hashmap_set::DbHashMapSet};
use std::collections::HashSet;
use tracing::*;
use uuid::Uuid;

use super::errors::LbResult;
use super::server_meta::ServerMeta;

pub struct ServerTree<'a> {
    pub ids: HashSet<Uuid>,
    pub owned_files: &'a mut DbHashMapSet<Owner, Uuid>,
    pub shared_files: &'a mut DbHashMapSet<Owner, Uuid>,
    pub file_children: &'a mut DbHashMapSet<Uuid, Uuid>,
    pub file_attachments: &'a mut DbHashMapSet<Uuid, Uuid>,
    pub files: &'a mut DbHashMap<Uuid, ServerMeta>,
}

impl<'a> ServerTree<'a> {
    pub fn new(
        owner: Owner, owned_files: &'a mut DbHashMapSet<Owner, Uuid>,
        shared_files: &'a mut DbHashMapSet<Owner, Uuid>,
        file_children: &'a mut DbHashMapSet<Uuid, Uuid>,
        file_attachments: &'a mut DbHashMapSet<Uuid, Uuid>,
        files: &'a mut DbHashMap<Uuid, ServerMeta>,
    ) -> LbResult<Self> {
        let (owned_ids, shared_ids) = match (owned_files.get(&owner), shared_files.get(&owner)) {
            (Some(owned_ids), Some(shared_ids)) => (owned_ids.clone(), shared_ids.clone()),
            _ => {
                warn!("Tree created for user without owned and shared files {:?}", owner);
                (HashSet::new(), HashSet::new())
            }
        };

        let mut ids = HashSet::new();
        ids.extend(owned_ids);
        ids.extend(shared_ids.clone());

        ids.extend(reachable(file_children, file_attachments, shared_ids));

        Ok(Self { ids, owned_files, shared_files, file_children, file_attachments, files })
    }

    /// The set of files whose access follows from the provided roots
    pub fn reachable(&self, roots: impl IntoIterator<Item = Uuid>) -> HashSet<Uuid> {
        reachable(self.file_children, self.file_attachments, roots)
    }
}

/// Walks each file once, however many roots reach it.
fn reachable(
    file_children: &DbHashMapSet<Uuid, Uuid>, file_attachments: &DbHashMapSet<Uuid, Uuid>,
    roots: impl IntoIterator<Item = Uuid>,
) -> HashSet<Uuid> {
    let mut result = HashSet::new();
    let mut expanded = HashSet::new();
    let mut to_visit = Vec::from_iter(roots);
    while let Some(id) = to_visit.pop() {
        if !expanded.insert(id) {
            continue;
        }
        let children = file_children.get(&id).into_iter().flatten();
        let attachments = file_attachments.get(&id).into_iter().flatten();
        for &file in children.chain(attachments) {
            if result.insert(file) {
                to_visit.push(file);
            }
        }
    }
    result
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
        let maybe_prior = DbHashMap::insert(self.files, id, f.clone())?;
        // the tree holds what's inserted into it, so what follows an upsert sees the files it
        // created as well as the ones it changed
        self.ids.insert(id);

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
        if self.file_children.get(&id).is_none() {
            self.file_children.create_key(id)?;
        }
        if self.file_children.get(f.parent()).is_none() {
            self.file_children.create_key(*f.parent())?;
        }
        if maybe_prior.as_ref().map(|f| *f.parent()) != Some(*f.parent()) {
            if let Some(ref prior) = maybe_prior {
                self.file_children.remove(prior.parent(), &id)?;
            }

            self.file_children.insert(*f.parent(), id)?;
        }

        // maintain index: file_attachments
        let prior_document = maybe_prior
            .as_ref()
            .and_then(|f| f.attachment_of())
            .map(|of| of.id);
        let document = f.attachment_of().map(|of| of.id);
        if prior_document != document {
            if let Some(prior_document) = prior_document {
                self.file_attachments.remove(&prior_document, &id)?;
            }
            if let Some(document) = document {
                self.file_attachments.insert(document, id)?;
            }
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
