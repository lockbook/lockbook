use crate::account::Account;
use crate::crypto::{AESEncrypted, AESKey};
use crate::{pubkey, symkey, SharedResult};
use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};

pub type EncryptedUserAccessKey = AESEncrypted<AESKey>;
pub type EncryptedFolderAccessKey = AESEncrypted<AESKey>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd, Ord)]
pub enum UserAccessMode {
    Read,
    Write,
    Owner,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserAccessInfo {
    pub mode: UserAccessMode,
    pub encrypted_by: PublicKey,
    pub encrypted_for: PublicKey,
    pub access_key: EncryptedUserAccessKey,
    pub deleted: bool,
}

impl PartialEq for UserAccessInfo {
    fn eq(&self, other: &Self) -> bool {
        self.encrypted_for == other.encrypted_for && self.encrypted_by == other.encrypted_by
    }
}

impl UserAccessInfo {
    pub fn encrypt(
        account: &Account, encrypted_by: &PublicKey, encrypted_for: &PublicKey, key: &AESKey,
    ) -> SharedResult<Self> {
        let private_key = account.private_key;
        let user_key = pubkey::get_aes_key(&private_key, encrypted_by)?;
        let encrypted_file_key = symkey::encrypt(&user_key, key)?;
        Ok(UserAccessInfo {
            mode: UserAccessMode::Owner,
            encrypted_by: *encrypted_by,
            encrypted_for: *encrypted_for,
            access_key: encrypted_file_key,
            deleted: false,
        })
    }
}
