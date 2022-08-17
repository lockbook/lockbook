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

pub struct ServerTree<'a, 'b, Log1, Log2>
where
    Log1: SchemaEvent<Owner, HashSet<Uuid>>,
    Log2: SchemaEvent<Uuid, ServerFile>,
{
    pub ids: HashSet<Uuid>,
    pub owned: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, Log1>,
    pub metas: &'a mut TransactionTable<'b, Uuid, ServerFile, Log2>,
}

impl<'a, 'b, Log1, Log2> ServerTree<'a, 'b, Log1, Log2>
where
    Log1: SchemaEvent<Owner, HashSet<Uuid>>,
    Log2: SchemaEvent<Uuid, ServerFile>,
{
    // todo: optimize/cache
    pub fn new(
        owner: Owner, owned: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, Log1>,
        metas: &'a mut TransactionTable<'b, Uuid, ServerFile, Log2>,
    ) -> SharedResult<Self> {
        let shared_ids = metas
            .get_all()
            .iter()
            .filter(|(_, f)| {
                f.user_access_keys()
                    .iter()
                    .any(|k| k.encrypted_for == owner.0)
            })
            .map(|(id, _)| *id)
            .collect::<HashSet<_>>();
        let mut tree = metas.to_lazy();
        let mut descendants_of_shared_ids = shared_ids.clone();
        for id in shared_ids {
            descendants_of_shared_ids.extend(tree.descendents(&id)?);
        }
        Ok(Self { ids: descendants_of_shared_ids, owned, metas })
    }
}

impl<'a, 'b, Log1, Log2> TreeLike for ServerTree<'a, 'b, Log1, Log2>
where
    Log1: SchemaEvent<Owner, HashSet<Uuid>>,
    Log2: SchemaEvent<Uuid, ServerFile>,
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
        let prior = TransactionTable::insert(self.metas, id, f);

        if prior == None {
            if let Some(mut owned) = self.owned.delete(owner) {
                owned.insert(id);
                self.owned.insert(owner, owned);
            } else {
                error!("File inserted with unknown owner")
            }
        }

        prior
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

impl<'a, 'b, Log1, Log2> Stagable for ServerTree<'a, 'b, Log1, Log2>
where
    Log1: SchemaEvent<Owner, HashSet<Uuid>>,
    Log2: SchemaEvent<Uuid, ServerFile>,
{
}
