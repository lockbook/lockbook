use crate::file_like::FileLike;
use crate::file_metadata::Owner;
use crate::server_file::ServerFile;
use crate::tree_like::{Stagable, TreeLike};
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
    pub owner: Owner,
    pub owned: &'a mut TransactionTable<'b, Owner, HashSet<Uuid>, Log1>,
    pub metas: &'a mut TransactionTable<'b, Uuid, ServerFile, Log2>,
}

impl<'a, 'b, Log1, Log2> TreeLike for ServerTree<'a, 'b, Log1, Log2>
where
    Log1: SchemaEvent<Owner, HashSet<Uuid>>,
    Log2: SchemaEvent<Uuid, ServerFile>,
{
    type F = ServerFile;

    fn ids(&self) -> HashSet<&Uuid> {
        let mut set = HashSet::new();
        if let Some(owned) = self.owned.get(&self.owner) {
            for item in owned {
                set.insert(item);
            }
        } else {
            error!("Server tree created without known owner")
        }
        set
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        if let Some(owned) = self.owned.get(&self.owner) {
            if !owned.contains(id) {
                return None;
            }
        } else {
            error!("Server tree created without known owner")
        }

        self.metas.get(id)
    }

    fn insert(&mut self, f: Self::F) -> Option<Self::F> {
        let id = *f.id();
        let prior = TransactionTable::insert(self.metas, id, f);

        if prior == None {
            if let Some(mut owned) = self.owned.delete(self.owner) {
                owned.insert(id);
                self.owned.insert(self.owner, owned);
            } else {
                error!("Server tree created without known owner")
            }
        }

        prior
    }

    fn remove(&mut self, id: Uuid) -> Option<Self::F> {
        error!("remove metadata called in server!");
        if let Some(owned) = self.owned.get(&self.owner) {
            let mut new_owned = owned.clone();
            let removed = new_owned.remove(&id);
            self.owned.insert(self.owner, new_owned);
            if removed {
                return self.metas.delete(id);
            }
        } else {
            error!("Server tree created without known owner")
        }

        None
    }
}

impl<'a, 'b, Log1, Log2> Stagable for ServerTree<'a, 'b, Log1, Log2>
where
    Log1: SchemaEvent<Owner, HashSet<Uuid>>,
    Log2: SchemaEvent<Uuid, ServerFile>,
{
}
