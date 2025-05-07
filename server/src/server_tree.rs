use tokio::sync::RwLockWriteGuard;
use uuid::Uuid;

use lb_rs::model::{
    file_metadata::Owner,
    server_meta::ServerMeta,
    tree_like::{TreeLike, TreeLikeMut},
};

use crate::schema::AccountV1;

pub struct ServerTreeV2<'a> {
    pub owner: Owner,
    pub trees: Vec<RwLockWriteGuard<'a, AccountV1>>,
}

impl TreeLike for ServerTreeV2<'_> {
    type F = ServerMeta;

    fn ids(&self) -> Vec<Uuid> {
        let mut ids = vec![];
        for tree in &self.trees {
            ids.extend_from_slice(&tree.metas.ids());
        }
        ids
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        for tree in &self.trees {
            if let Some(meta) = tree.metas.get().get(id) {
                return Some(meta);
            }
        }

        None
    }
}

impl TreeLikeMut for ServerTreeV2<'_> {
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
