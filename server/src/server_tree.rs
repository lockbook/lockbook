use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use itertools::Itertools;
use tokio::sync::OwnedRwLockWriteGuard;
use uuid::Uuid;

use lb_rs::{
    model::{
        clock::get_time,
        errors::DiffError,
        file_like::FileLike,
        file_metadata::{FileDiff, Owner},
        lazy::{LazyStaged1, LazyTree},
        server_meta::{IntoServerMeta, ServerMeta},
        signed_file::SignedFile,
        signed_meta::SignedMeta,
        tree_like::{TreeLike, TreeLikeMut},
    },
    LbErrKind, LbResult,
};

use crate::{
    billing::{
        app_store_client::AppStoreClient, google_play_client::GooglePlayClient,
        stripe_client::StripeClient,
    },
    document_service::DocumentService,
    schema::AccountV1,
    MetaLookup, ServerError, ServerState,
};

// todo: is it worthwhile to have a Mut variant and make this read only?
// If not, should these just all be normal mutexes instead of RwLocks
pub struct ServerTreeV2 {
    pub owner: Owner,
    pub owner_db: OwnedRwLockWriteGuard<AccountV1>,
    pub ids: Vec<Uuid>,
    // todo: ensure the owner cannot be in here (maliciously crafted input)
    pub sharee_dbs: HashMap<Owner, OwnedRwLockWriteGuard<AccountV1>>,
    pub meta_lookup: MetaLookup,
}

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    // todo -- at what point should this tree begin_tx's on the databases
    // todo -- this needs to take a diff and also add newly shared people to this,
    // this is so that when upsert accepts a new share it can update their db. Their
    // ids are not added here because you can't just add a sharee and get access to
    // their ids. But the insert logic will be able to ensure that you're not adding
    // an id they already have in their db. It will also subsequently be able to
    // update their secondary index about shares during change promotion
    //
    // we're actually just going to use a secondary index to ensure that new ids are
    // truly new, makes a lot of complexity go away
    pub async fn get_tree<T: Debug>(
        &self, owner: Owner, req_sharees: Vec<Owner>,
    ) -> Result<ServerTreeV2, ServerError<T>> {
        let owner_dbs = self.account_dbs.read().await;

        // grab our requester's db
        let owner_db = owner_dbs.get(&owner).unwrap().clone().write_owned().await;
        let mut ids = owner_db.metas.ids();

        // get all relevant sharee dbs and sort for determinism
        let mut owners = req_sharees;
        for (owner, _ids) in owner_db.shared_files.get() {
            owners.push(*owner);
        }
        owners.sort_unstable_by_key(|owner| owner.0.serialize());
        owners.dedup();

        // aquire locks and find compute the requester's set of ids
        let mut sharee_dbs = HashMap::new();
        for owner in owners {
            let db = owner_dbs.get(&owner).unwrap().clone();
            let db = db.write_owned().await;
            let mut temp_tree = db.metas.get().to_lazy();
            let shared_ids = owner_db.shared_files.get().get(&owner).unwrap();
            for id in shared_ids {
                let desc = temp_tree
                    .descendants(id)
                    .map_err(|e| {
                        ServerError::InternalError(format!(
                            "Could not compute desc {id}, {owner:?} err: {e:?}"
                        ))
                    })?
                    .into_iter()
                    .collect_vec(); // todo: can prob remove
                ids.extend_from_slice(&desc);
            }

            sharee_dbs.insert(owner, db);
        }

        // return the tree with all the metadata to fulfill requests
        Ok(ServerTreeV2 { owner, owner_db, ids, sharee_dbs, meta_lookup: self.meta_lookup.clone() })
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

impl ServerTreeV2 {
    fn find_owner_db(
        &mut self, owner_db: &Owner,
    ) -> LbResult<&mut OwnedRwLockWriteGuard<AccountV1>> {
        if owner_db == &self.owner {
            return Ok(&mut self.owner_db);
        } else {
            return self.sharee_dbs.get_mut(owner_db).ok_or(
                LbErrKind::Unexpected(format!("Owner not found for ServerTree insertion")).into(),
            );
        }
    }
}

impl TreeLikeMut for ServerTreeV2 {
    fn insert(&mut self, f: Self::F) -> LbResult<Option<Self::F>> {
        let id = *f.id();
        let owner = f.owner();
        let maybe_prior = self.remove(id)?;
        self.find_owner_db(&owner)?.metas.insert(id, f.clone())?;

        // maintain index: meta_lookup
        if maybe_prior.as_ref().map(|f| f.owner()) != Some(f.owner()) {
            self.meta_lookup.lock().unwrap().insert(id, owner);
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

        // handle owners changing
        if let Some(prior) = &maybe_prior {
            if prior.owner() != owner {
                for sharee in &prior_sharees {
                    let db = self.find_owner_db(sharee)?;
                    let mut entry =
                        db.shared_files
                            .clear_key(&prior.owner())?
                            .ok_or(LbErrKind::Unexpected(format!(
                                "could not find entry in shared dbs for owner"
                            )))?;
                    entry.retain(|shared_id| *shared_id != id);
                    for shared_id in entry {
                        db.shared_files.push(owner, shared_id)?;
                    }
                }
            }
        }

        // handle sharees changing
        for removed_sharee in prior_sharees.difference(&sharees) {
            let db = self.find_owner_db(removed_sharee)?;
            let mut entry = db
                .shared_files
                .clear_key(&owner)?
                .ok_or(LbErrKind::Unexpected(format!(
                    "could not find entry in shared dbs for owner"
                )))?;
            entry.retain(|shared_id| *shared_id != id);
            for shared_id in entry {
                db.shared_files.push(owner, shared_id)?;
            }
        }
        for new_sharee in sharees.difference(&prior_sharees) {
            let db = self.find_owner_db(new_sharee)?;
            db.shared_files.push(owner, id)?;
        }

        Ok(maybe_prior)
    }

    fn remove(&mut self, id: Uuid) -> LbResult<Option<Self::F>> {
        match self.maybe_find(&id).map(|f| f.owner()) {
            Some(owner) => {
                let db = self.find_owner_db(&owner)?;
                Ok(db.metas.remove(&id)?)
            }
            None => Ok(None),
        }
    }

    fn clear(&mut self) -> crate::LbResult<()> {
        todo!("no one uses this on server yet")
    }
}

type LazyServerStaged1 = LazyStaged1<ServerTreeV2, Vec<ServerMeta>>;

pub trait ServerTreeOps {
    /// Validates a diff prior to staging it. Performs individual validations, then validations that
    /// require a tree
    fn stage_diff(self, changes: Vec<FileDiff<SignedMeta>>) -> LbResult<LazyServerStaged1>;
}

impl ServerTreeOps for LazyTree<ServerTreeV2> {
    fn stage_diff(self, changes: Vec<FileDiff<SignedMeta>>) -> LbResult<LazyServerStaged1> {
        // Check new.id == old.id
        for change in &changes {
            if let Some(old) = &change.old {
                if old.id() != change.new.id() {
                    return Err(LbErrKind::Diff(DiffError::DiffMalformed))?;
                }
            }
        }

        // Check for changes to digest
        for change in &changes {
            match &change.old {
                Some(old) => {
                    if old.timestamped_value.value.document_hmac()
                        != change.new.timestamped_value.value.document_hmac()
                    {
                        return Err(LbErrKind::Diff(DiffError::HmacModificationInvalid))?;
                    }
                }
                None => {
                    if change.new.timestamped_value.value.document_hmac().is_some() {
                        return Err(LbErrKind::Diff(DiffError::HmacModificationInvalid))?;
                    }
                }
            }
        }

        // Check for race conditions
        for change in &changes {
            match &change.old {
                Some(old) => {
                    let current = &self
                        .maybe_find(old.id())
                        .ok_or(LbErrKind::Diff(DiffError::OldFileNotFound))?
                        .file;
                    if current != old {
                        return Err(LbErrKind::Diff(DiffError::OldVersionIncorrect))?;
                    }
                }
                None => {
                    // if you're claiming this file is new, it must be globally unique
                    if self
                        .tree
                        .meta_lookup
                        .lock()
                        .unwrap()
                        .contains_key(change.id())
                    {
                        return Err(LbErrKind::Diff(DiffError::OldVersionRequired))?;
                    }
                }
            }
        }

        let now = get_time().0 as u64;
        let changes = changes
            .into_iter()
            .map(|change| change.new.add_time(now))
            .collect();

        Ok(self.stage(changes))
    }
}
