use crate::Lb;
use crate::model::api::GetPublicKeyRequest;
use crate::model::errors::{LbErr, LbResult};
use crate::model::file::{File, ShareMode};
use crate::model::file_like::FileLike;
use crate::model::file_metadata::Owner;
use crate::model::tree_like::TreeLike;
use libsecp256k1::PublicKey;
use uuid::Uuid;

impl Lb {
    // todo: this can check whether the username is known already
    #[instrument(level = "debug", skip(self))]
    pub async fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()> {
        let account = self.get_account()?;
        let username = username.to_lowercase();

        let sharee = Owner(
            self.client
                .request(account, GetPublicKeyRequest { username: username.clone() })
                .await
                .map_err(LbErr::from)?
                .key,
        );

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        db.pub_key_lookup.insert(sharee, username)?;

        tree.add_share(id, sharee, mode, &self.keychain)?;

        tx.end();

        self.events.meta_changed();

        Ok(())
    }

    // todo: move to tree
    #[instrument(level = "debug", skip(self))]
    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let owner = Owner(self.keychain.get_pk()?);
        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let mut result = Vec::new();
        for id in tree.ids() {
            // file must be owned by another user
            if tree.find(&id)?.owner() == owner {
                continue;
            }

            // file must be shared with this user
            if tree.find(&id)?.access_mode(&owner).is_none() {
                continue;
            }

            // file must not be deleted
            if tree.calculate_deleted(&id)? {
                continue;
            }

            // file must not have any links pointing to it
            if tree.linked_by(&id)?.is_some() {
                continue;
            }

            let file = tree.decrypt(&self.keychain, &id, &db.pub_key_lookup)?;

            result.push(file);
        }
        Ok(result)
    }

    #[instrument(level = "debug", skip(self))]
    async fn delete_share(
        &self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>,
    ) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();

        tree.delete_share(id, maybe_encrypted_for, &self.keychain)?;

        tx.end();
        self.events.meta_changed();

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn known_usernames(&self) -> LbResult<Vec<String>> {
        let db = self.ro_tx().await;
        let db = db.db();

        Ok(db.pub_key_lookup.get().values().cloned().collect())
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn reject_share(&self, id: &Uuid) -> Result<(), LbErr> {
        let pk = self.keychain.get_pk()?;
        self.delete_share(id, Some(pk)).await
    }
}
