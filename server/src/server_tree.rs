use std::collections::HashMap;
use std::fmt::Debug;

use itertools::Itertools;
use tokio::sync::OwnedRwLockWriteGuard;
use uuid::Uuid;

use lb_rs::model::{
    file_metadata::Owner,
    server_meta::ServerMeta,
    tree_like::{TreeLike, TreeLikeMut},
};

use crate::{
    billing::{
        app_store_client::AppStoreClient, google_play_client::GooglePlayClient,
        stripe_client::StripeClient,
    },
    document_service::DocumentService,
    schema::AccountV1,
    ServerError, ServerState,
};

// todo: is it worthwhile to have a Mut variant and make this read only?
pub struct ServerTreeV2 {
    pub owner: Owner,
    pub owner_db: OwnedRwLockWriteGuard<AccountV1>,
    pub ids: Vec<Uuid>,
    pub sharee_dbs: HashMap<Owner, OwnedRwLockWriteGuard<AccountV1>>,
}

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub async fn get_tree<T: Debug>(&self, owner: Owner) -> Result<ServerTreeV2, ServerError<T>> {
        let owner_dbs = self.account_dbs.read().await;

        // grab our requester's db
        let owner_db = owner_dbs.get(&owner).unwrap().clone().write_owned().await;
        let mut ids = owner_db.metas.ids();

        // get all relevant sharee dbs and sort for determinism
        let mut owners = vec![];
        for (owner, _ids) in owner_db.shared_files.get() {
            owners.push(owner);
        }
        owners.sort_unstable_by_key(|owner| owner.0.serialize());

        // aquire locks and find compute the requester's set of ids
        let mut sharee_dbs = HashMap::new();
        for owner in owners {
            let db = owner_dbs.get(&owner).unwrap().clone();
            let db = db.write_owned().await;
            let mut temp_tree = db.metas.get().to_lazy();
            let shared_ids = owner_db.shared_files.get().get(owner).unwrap();
            for id in shared_ids {
                let desc = temp_tree
                    .descendants(id)
                    .map_err(|e| {
                        ServerError::InternalError(format!(
                            "Could not compute desc {id}, {owner:?} err: {e:?}"
                        ))
                    })?
                    .into_iter()
                    .collect_vec();
                ids.extend_from_slice(&desc);
            }

            sharee_dbs.insert(*owner, db);
        }

        // return the tree with all the metadata to fulfill requests
        Ok(ServerTreeV2 { owner, owner_db, ids, sharee_dbs })
    }
}

impl TreeLike for ServerTreeV2 {
    type F = ServerMeta;

    fn ids(&self) -> Vec<Uuid> {
        self.ids.clone()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        // limit access to the only the ids this person is supposed to be able to see
        if !self.ids.contains(id) {
            return None;
        }

        match self.owner_db.metas.get().get(id) {
            Some(f) => return Some(f),
            None => {
                for (_owner, tree) in &self.sharee_dbs {
                    if let Some(meta) = tree.metas.get().get(id) {
                        return Some(meta);
                    }
                }
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
