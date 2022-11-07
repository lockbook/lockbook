use crate::{CoreError, CoreResult, OneKey, RequestContext, Requester};
use libsecp256k1::PublicKey;
use lockbook_shared::access_info::{UserAccessInfo, UserAccessMode};
use lockbook_shared::api::GetPublicKeyRequest;
use lockbook_shared::file::{File, ShareMode};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::lazy::LazyTreeLike;
use lockbook_shared::tree_like::{TreeLike, TreeLikeMut};
use lockbook_shared::validate;
use uuid::Uuid;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    // todo: move to tree, split non-validating version
    pub fn share_file(&mut self, id: Uuid, username: &str, mode: ShareMode) -> CoreResult<()> {
        let account = &self.get_account()?.clone(); // todo: don't clone
        let owner = Owner(self.get_public_key()?);
        let access_mode = match mode {
            ShareMode::Write => UserAccessMode::Write,
            ShareMode::Read => UserAccessMode::Read,
        };
        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let mut file = tree.find(&id)?.timestamped_value.value.clone();
        if tree.calculate_deleted(&id)? {
            return Err(CoreError::FileNonexistent);
        }
        validate::not_root(&file)?;
        if mode == ShareMode::Write && file.owner.0 != owner.0 {
            return Err(CoreError::InsufficientPermission);
        }
        // check for and remove duplicate shares
        let mut found = false;
        let sharee_public_key = self
            .client
            .request(account, GetPublicKeyRequest { username: String::from(username) })
            .map_err(CoreError::from)?
            .key;
        for user_access in &mut file.user_access_keys {
            if user_access.encrypted_for == sharee_public_key {
                found = true;
                if user_access.mode == access_mode && !user_access.deleted {
                    return Err(CoreError::ShareAlreadyExists);
                }
            }
        }
        if found {
            file.user_access_keys = file
                .user_access_keys
                .into_iter()
                .filter(|k| k.encrypted_for != sharee_public_key)
                .collect();
        }
        file.user_access_keys.push(UserAccessInfo::encrypt(
            account,
            &owner.0,
            &sharee_public_key,
            &tree.decrypt_key(&id, account)?,
            access_mode,
        )?);

        let mut tree = tree.stage_lazy(Some(file.sign(account)?));
        tree = tree.validate(Owner(account.public_key()))?;
        tree.promote();

        self.tx
            .public_key_by_username
            .insert(String::from(username), Owner(sharee_public_key));
        self.tx
            .username_by_public_key
            .insert(Owner(sharee_public_key), String::from(username));

        Ok(())
    }

    // todo: move to tree, split non-validating version
    pub fn get_pending_shares(&mut self) -> CoreResult<Vec<File>> {
        let account = &self.get_account()?.clone(); // todo: don't clone
        let owner = Owner(self.get_public_key()?);
        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
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
        let tree = self
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
