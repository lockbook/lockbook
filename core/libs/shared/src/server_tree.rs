use crate::file_like::FileLike;
use crate::file_metadata::Owner;
use crate::server_file::ServerFile;
use crate::tree_like::{TreeLike, TreeLikeMut};
use crate::SharedResult;
use hmdb::log::SchemaEvent;
use hmdb::transaction::TransactionTable;
use std::collections::HashSet;
use std::iter::FromIterator;
use tracing::*;
use uuid::Uuid;

pub struct ServerTree<'a, 'b, OwnedFiles, SharedFiles, FileChildren, Files>
where
    OwnedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    SharedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    FileChildren: SchemaEvent<Uuid, HashSet<Uuid>>,
    Files: SchemaEvent<Uuid, ServerFile>,
{
    pub ids: HashSet<Uuid>,
    pub owned_files: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, OwnedFiles>,
    pub shared_files: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, SharedFiles>,
    pub file_children: &'a mut TransactionTable<'b, Uuid, HashSet<Uuid>, FileChildren>,
    pub files: &'a mut TransactionTable<'b, Uuid, ServerFile, Files>,
}

impl<'a, 'b, OwnedFiles, SharedFiles, FileChildren, Files>
    ServerTree<'a, 'b, OwnedFiles, SharedFiles, FileChildren, Files>
where
    OwnedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    SharedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    FileChildren: SchemaEvent<Uuid, HashSet<Uuid>>,
    Files: SchemaEvent<Uuid, ServerFile>,
{
    pub fn new(
        owner: Owner, owned_files: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, OwnedFiles>,
        shared_files: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, SharedFiles>,
        file_children: &'a mut TransactionTable<'b, Uuid, HashSet<Uuid>, FileChildren>,
        files: &'a mut TransactionTable<'b, Uuid, ServerFile, Files>,
    ) -> SharedResult<Self> {
        let (owned_ids, shared_ids) = match (owned_files.get(&owner), shared_files.get(&owner)) {
            (Some(owned_ids), Some(shared_ids)) => (owned_ids.clone(), shared_ids.clone()),
            _ => {
                error!("Tree created for user without owned and shared files");
                (HashSet::new(), HashSet::new())
            }
        };

        let mut ids = HashSet::new();
        ids.extend(owned_ids);
        ids.extend(shared_ids.clone());

        let mut to_get_descendants = Vec::from_iter(shared_ids);
        while let Some(id) = to_get_descendants.pop() {
            let children = file_children.get(&id).cloned().unwrap_or_default();
            ids.extend(children.clone());
            to_get_descendants.extend(children);
        }

        Ok(Self { ids, owned_files, shared_files, file_children, files })
    }
}

impl<OwnedFiles, SharedFiles, FileChildren, Files> TreeLike
    for ServerTree<'_, '_, OwnedFiles, SharedFiles, FileChildren, Files>
where
    FileChildren: SchemaEvent<Uuid, HashSet<Uuid>>,
    Files: SchemaEvent<Uuid, ServerFile>,
    OwnedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    SharedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
{
    type F = ServerFile;

    fn ids(&self) -> HashSet<&Uuid> {
        self.ids.iter().collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        if self.ids.contains(id) {
            self.files.maybe_find(id)
        } else {
            None
        }
    }
}

impl<OwnedFiles, SharedFiles, FileChildren, Files> TreeLikeMut
    for ServerTree<'_, '_, OwnedFiles, SharedFiles, FileChildren, Files>
where
    OwnedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    SharedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    FileChildren: SchemaEvent<Uuid, HashSet<Uuid>>,
    Files: SchemaEvent<Uuid, ServerFile>,
{
    fn insert(&mut self, f: Self::F) -> Option<Self::F> {
        let id = *f.id();
        let owner = f.owner();
        let maybe_prior = TransactionTable::insert(self.files, id, f.clone());

        // maintain index: owned_files
        if maybe_prior.as_ref().map(|f| f.owner()) != Some(f.owner()) {
            if let Some(ref prior) = maybe_prior {
                if let Some(mut owned) = self.owned_files.delete(prior.owner()) {
                    owned.remove(&id);
                    self.owned_files.insert(prior.owner(), owned);
                } else {
                    error!("File inserted with unknown prior owner")
                }
            }
            if let Some(mut owned) = self.owned_files.delete(owner) {
                owned.insert(id);
                self.owned_files.insert(owner, owned);
            } else {
                error!("File inserted with unknown owner")
            }
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
            if let Some(mut shared) = self.shared_files.delete(*removed_sharee) {
                shared.remove(&id);
                self.shared_files.insert(*removed_sharee, shared);
            } else {
                error!("File inserted with unknown prior sharee")
            }
        }
        for new_sharee in sharees.difference(&prior_sharees) {
            if let Some(mut shared) = self.shared_files.delete(*new_sharee) {
                shared.insert(id);
                self.shared_files.insert(*new_sharee, shared);
            } else {
                error!("File inserted with unknown sharee")
            }
        }

        // maintain index: file_children
        if self.file_children.get(&id).is_none() {
            self.file_children.insert(id, HashSet::new());
        }
        if self.file_children.get(f.parent()).is_none() {
            self.file_children.insert(*f.parent(), HashSet::new());
        }
        if maybe_prior.as_ref().map(|f| *f.parent()) != Some(*f.parent()) {
            if let Some(ref prior) = maybe_prior {
                if let Some(mut children) = self.file_children.delete(*prior.parent()) {
                    children.remove(&id);
                    self.file_children.insert(*prior.parent(), children);
                }
            }
            if let Some(mut children) = self.file_children.delete(*f.parent()) {
                children.insert(id);
                self.file_children.insert(*f.parent(), children);
            }
        }

        maybe_prior
    }

    fn remove(&mut self, _id: Uuid) -> Option<Self::F> {
        error!("remove metadata called in server!");
        None
    }
}
