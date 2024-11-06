use crate::logic::file_like::FileLike;
use crate::logic::tree_like::TreeLike;
use crate::model::api::GetPublicKeyRequest;
use crate::model::errors::{LbErr, LbResult};
use crate::model::file::{File, ShareMode};
use crate::model::file_metadata::Owner;
use crate::Lb;
use libsecp256k1::PublicKey;
use uuid::Uuid;

impl Lb {
    // todo: this can check whether the username is known already
    pub async fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()> {
        let account = self.get_account()?;

        let sharee = Owner(
            self.client
                .request(account, GetPublicKeyRequest { username: String::from(username) })
                .await
                .map_err(LbErr::from)?
                .key,
        );

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        db.pub_key_lookup.insert(sharee, String::from(username))?;

        tree.add_share(id, sharee, mode, account)?;

        self.spawn_build_index();

        Ok(())
    }

    // todo: move to tree
    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let account = &self.get_account()?.clone(); // todo: don't clone
        let owner = Owner(self.get_pk()?);
        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let mut result = Vec::new();
        for id in tree.owned_ids() {
            // file must not be deleted
            if tree.calculate_deleted(&id)? {
                continue;
            }
            // file must be owned by another user
            if tree.find(&id)?.owner() == owner {
                continue;
            }
            // file must be shared with this user
            if tree.find(&id)?.access_mode(&owner).is_none() {
                continue;
            }
            // file must not have any links pointing to it
            if tree.linked_by(&id)?.is_some() {
                continue;
            }

            let file = tree.decrypt(account, &id, &db.pub_key_lookup)?;

            result.push(file);
        }
        Ok(result)
    }

    async fn delete_share(
        &self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>,
    ) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        let account = self.get_account()?;

        tree.delete_share(id, maybe_encrypted_for, account)?;

        self.spawn_build_index();

        Ok(())
    }

    pub async fn reject_share(&self, id: &Uuid) -> Result<(), LbErr> {
        let pk = self.get_account()?.public_key();
        let result = self.delete_share(id, Some(pk)).await;

        self.spawn_build_index();

        result
    }
}
