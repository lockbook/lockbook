use lockbook_shared::access_info::{UserAccessInfo, UserAccessMode};
use lockbook_shared::api::GetPublicKeyRequest;
use lockbook_shared::file::{File, ShareMode};
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::lazy::LazyStaged1;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::validate;
use uuid::Uuid;

use crate::service::api_service;
use crate::{CoreError, CoreResult, RequestContext};

impl RequestContext<'_, '_> {
    pub fn share_file(&mut self, id: Uuid, username: &str, mode: ShareMode) -> CoreResult<()> {
        let account = &self.get_account()?.clone(); // todo: don't clone
        let owner = Owner(self.get_public_key()?);
        let access_mode = match mode {
            ShareMode::Write => UserAccessMode::Write,
            ShareMode::Read => UserAccessMode::Read,
        };

        let mut tree =
            LazyStaged1::core_tree(owner, &mut self.tx.base_metadata, &mut self.tx.local_metadata);
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
        for user_access in &mut file.user_access_keys {
            if user_access.encrypted_for == owner.0 {
                found = true;
                if user_access.mode == access_mode {
                    return Err(CoreError::ShareAlreadyExists);
                } else {
                    user_access.mode = access_mode;
                }
            }
        }
        if !found {
            let sharee_public_key = api_service::request(
                &account,
                GetPublicKeyRequest { username: String::from(username) },
            )
            .map_err(CoreError::from)?
            .key;
            file.user_access_keys.push(UserAccessInfo::encrypt(
                account,
                &owner.0,
                &sharee_public_key,
                &tree.decrypt_key(&id, account)?,
            )?);
        }

        let file = file.sign(account)?;
        let mut tree = tree.stage(Some(file));
        tree.validate()?;
        tree.promote();

        Ok(())
    }

    pub fn get_pending_shares(&mut self) -> CoreResult<Vec<File>> {
        todo!()
    }

    pub fn delete_pending_share(&mut self, _id: Uuid) -> CoreResult<()> {
        todo!()
    }
}
