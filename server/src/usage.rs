use std::{collections::HashMap, iter};

use lb_rs::{
    model::{api::METADATA_FEE, file_like::FileLike, file_metadata::Owner, tree_like::TreeLike},
    LbResult,
};
use tracing::error;
use uuid::Uuid;

use crate::{
    schema::{AccountV1, ServerV5},
    server_tree::ServerTreeV2,
};

pub struct UsageReport {
    pub caps: HashMap<Owner, u64>,
    pub space_used: HashMap<Owner, u64>,
    pub sizes: HashMap<Uuid, u64>,
}

impl ServerTreeV2 {
    pub fn get_caps(&self, db: &ServerV5) -> HashMap<Owner, u64> {
        self.sharee_dbs
            .keys()
            .copied()
            .chain(iter::once(self.owner)) // an iter with all the owners
            .map(|owner| {
                (
                    owner, // (owner, cap) for HashMap::collect
                    db.accounts
                        .get()
                        .get(&owner)
                        .map(|account| account.billing_info.data_cap()) // lookup each person's data cap
                        .unwrap_or_else(|| {
                            error!("cap missing for {owner:?}"); // page if not found
                            0 // conservative default
                        }),
                )
            })
            .collect()
    }

    /// There are two operations we evaluate space for: upsert and doc edits.
    ///
    /// During an upsert id ownership can change, the contents of
    /// a document can be deleted, and metadata can be created
    ///
    /// During a doc edit the content of a document can change
    ///
    /// For both of these operations you just need to construct a single tree,
    /// but to understand the space implications for any of these operations
    /// you need to construct a tree for each of the sharees that may be involved
    /// in an operation
    ///
    /// Fortunately due to the nature of sharing, we probably don't need to construct
    /// extended trees (with sharees as well), we can construct naive trees.
    ///
    /// These naive trees won't pass validation, because they'll have broken links
    /// but we don't need them to pass validations to compute their size.
    ///
    /// Looking at the present api, it seems like superset of extended trees isn'
    /// nessisarily a problem, which means we won't have broken links. It could
    /// be more expensive for certain operations as ids() would return a lot of
    /// files in the worst case.
    pub fn usage_report(&self, caps: HashMap<Owner, u64>) -> LbResult<UsageReport> {
        let mut sizes = HashMap::new();
        let mut space_used = HashMap::new();

        // process owner first
        let total = Self::owner_usage(&self.owner_db, &mut sizes)?;
        space_used.insert(self.owner, total);

        // process other sharees
        for (sharee, sharee_db) in &self.sharee_dbs {
            let total = Self::owner_usage(sharee_db, &mut sizes)?;
            space_used.insert(*sharee, total);
        }

        Ok(UsageReport { caps, space_used, sizes })
    }

    fn owner_usage(db: &AccountV1, sizes: &mut HashMap<Uuid, u64>) -> LbResult<u64> {
        let mut size_sum = 0;

        let mut tree = db.metas.get().to_lazy();
        for id in tree.ids() {
            let size = if !tree.calculate_deleted(&id)? && tree.find(&id)?.is_document() {
                db.sizes.get().get(&id).copied().unwrap_or_else(|| {
                    error!("could not retrieve size for id: {id}");
                    0
                })
            } else {
                METADATA_FEE
            };

            size_sum += size;
            sizes.insert(id, size);
        }

        Ok(size_sum)
    }
}

impl UsageReport {
    pub fn allowed(
        &self, new: &[(Owner, Uuid)], deleted: &[(Owner, Uuid)], doc_changes: &[(Owner, u64)],
    ) -> bool {
        todo!()
    }
}
