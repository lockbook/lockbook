use crate::file_like::FileLike;
use crate::file_metadata::Owner;
use crate::server_file::ServerFile;
use crate::tree_like::{Stagable, TreeLike};
use crate::SharedResult;
use hmdb::log::SchemaEvent;
use hmdb::transaction::TransactionTable;
use std::collections::HashSet;
use tracing::*;
use uuid::Uuid;

pub struct ServerTree<'a, 'b, OwnedFiles, SharedFiles, Files>
where
    OwnedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    SharedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    Files: SchemaEvent<Uuid, ServerFile>,
{
    pub ids: HashSet<Uuid>,
    pub owned: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, OwnedFiles>,
    pub shared: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, SharedFiles>,
    pub metas: &'a mut TransactionTable<'b, Uuid, ServerFile, Files>,
}

impl<'a, 'b, OwnedFiles, SharedFiles, Files> ServerTree<'a, 'b, OwnedFiles, SharedFiles, Files>
where
    OwnedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    SharedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    Files: SchemaEvent<Uuid, ServerFile>,
{
    // todo: optimize/cache
    pub fn new(
        owner: Owner, owned: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, OwnedFiles>,
        shared: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, SharedFiles>,
        metas: &'a mut TransactionTable<'b, Uuid, ServerFile, Files>,
    ) -> SharedResult<Self> {
        let mut tree = metas.to_lazy();
        let (owned_ids, shared_ids) = match (owned.get(&owner), shared.get(&owner)) {
            (Some(owned_ids), Some(shared_ids)) => (owned_ids.clone(), shared_ids.clone()),
            _ => {
                error!("Tree created for user without owned and shared files");
                (HashSet::new(), HashSet::new())
            }
        };

        let mut ids = HashSet::new();
        ids.extend(owned_ids);
        ids.extend(shared_ids.clone());
        for id in shared_ids {
            ids.extend(tree.descendants(&id)?);
        }

        Ok(Self { ids, owned, shared, metas })
    }
}

impl<'a, 'b, OwnedFiles, SharedFiles, Files> TreeLike
    for ServerTree<'a, 'b, OwnedFiles, SharedFiles, Files>
where
    OwnedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    SharedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    Files: SchemaEvent<Uuid, ServerFile>,
{
    type F = ServerFile;

    fn ids(&self) -> HashSet<&Uuid> {
        self.ids.iter().collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        if self.ids.contains(id) {
            self.metas.maybe_find(id)
        } else {
            None
        }
    }

    fn insert(&mut self, f: Self::F) -> Option<Self::F> {
        let id = *f.id();
        let owner = f.owner();
        let maybe_prior = TransactionTable::insert(self.metas, id, f.clone());

        // maintain index: owned_files
        if maybe_prior.is_none() {
            if let Some(mut owned) = self.owned.delete(owner) {
                owned.insert(id);
                self.owned.insert(owner, owned);
            } else {
                error!("File inserted with unknown owner")
            }
        }

        // maintain index: shared_files
        let prior_sharees = if let Some(prior) = maybe_prior.clone() {
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
            if let Some(mut shared) = self.shared.delete(*removed_sharee) {
                shared.remove(&id);
                self.shared.insert(*removed_sharee, shared);
            } else {
                error!("File inserted with unknown sharee")
            }
        }
        for new_sharee in sharees.difference(&prior_sharees) {
            if let Some(mut shared) = self.shared.delete(*new_sharee) {
                shared.insert(id);
                self.shared.insert(*new_sharee, shared);
            } else {
                error!("File inserted with unknown sharee")
            }
        }

        maybe_prior
    }

    fn remove(&mut self, id: Uuid) -> Option<Self::F> {
        error!("remove metadata called in server!");
        let result = self.metas.delete(id);
        if let Some(deleted) = result {
            if let Some(owned) = self.owned.get(&deleted.owner()) {
                let mut new_owned = owned.clone();
                let removed = new_owned.remove(&id);
                self.owned.insert(deleted.owner(), new_owned);
                if removed {
                    return self.metas.delete(id);
                }
            } else {
                error!("File removed with unknown owner")
            }
            Some(deleted)
        } else {
            None
        }
    }
}

impl<'a, 'b, OwnedFiles, SharedFiles, Files> Stagable
    for ServerTree<'a, 'b, OwnedFiles, SharedFiles, Files>
where
    OwnedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    SharedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    Files: SchemaEvent<Uuid, ServerFile>,
{
}
