use tokio::sync::OwnedRwLockWriteGuard;
use uuid::Uuid;

use lb_rs::model::{
    file_metadata::Owner,
    server_meta::ServerMeta,
    tree_like::{TreeLike, TreeLikeMut},
};

use crate::schema::AccountV1;

// todo: is it worthwhile to have a Mut variant and make this read only?
pub struct ServerTreeV2 {
    pub owner: Owner,
    pub trees: Vec<(Owner, OwnedRwLockWriteGuard<AccountV1>)>,
}

impl TreeLike for ServerTreeV2 {
    type F = ServerMeta;

    fn ids(&self) -> Vec<Uuid> {
        let mut ids = vec![];
        for (_owner, tree) in &self.trees {
            ids.extend_from_slice(&tree.metas.ids());
        }
        ids
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        for (_owner, tree) in &self.trees {
            if let Some(meta) = tree.metas.get().get(id) {
                return Some(meta);
            }
        }

        None
    }
}

impl TreeLikeMut for ServerTreeV2 {
    fn insert(&mut self, f: Self::F) -> crate::LbResult<Option<Self::F>> {
        todo!()
    }

    fn remove(&mut self, id: Uuid) -> crate::LbResult<Option<Self::F>> {
        todo!()
    }

    fn clear(&mut self) -> crate::LbResult<()> {
        todo!()
    }
}
