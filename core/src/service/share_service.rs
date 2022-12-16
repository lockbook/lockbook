use crate::{CoreError, CoreResult, OneKey, RequestContext, Requester};
use libsecp256k1::PublicKey;
use lockbook_shared::api::GetPublicKeyRequest;
use lockbook_shared::file::{File, ShareMode};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::tree_like::TreeLike;
use uuid::Uuid;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn share_file(&mut self, id: Uuid, username: &str, mode: ShareMode) -> CoreResult<()> {
        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let sharee = Owner(
            self.client
                .request(account, GetPublicKeyRequest { username: String::from(username) })
                .map_err(CoreError::from)?
                .key,
        );
        self.tx
            .public_key_by_username
            .insert(String::from(username), sharee);
        self.tx
            .username_by_public_key
            .insert(sharee, String::from(username));

        tree.add_share(id, sharee, mode, account)?;

        Ok(())
    }

    // todo: move to tree
    pub fn get_pending_shares(&mut self) -> CoreResult<Vec<File>> {
        let account = &self.get_account()?.clone(); // todo: don't clone
        let owner = Owner(self.get_public_key()?);
        let mut tree = self
            .tx
            .base_metadata
            .stage(&self.tx.local_metadata)
            .to_lazy();

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
            if tree.link(&id)?.is_some() {
                continue;
            }

            result.push(tree.finalize(&id, account, &mut self.tx.username_by_public_key)?);
        }
        Ok(result)
    }

    pub fn delete_share(
        &mut self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>,
    ) -> CoreResult<()> {
        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        tree.delete_share(id, maybe_encrypted_for, account)?;
        Ok(())
    }
}
