use crate::account::{Account, Username};
use crate::crypto::{AESEncrypted, AESKey};
use crate::{pubkey, symkey, SharedResult};
use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type EncryptedUserAccessKey = AESEncrypted<AESKey>;
pub type EncryptedFolderAccessKey = AESEncrypted<AESKey>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UserAccessInfo {
    pub username: String,
    pub encrypted_by: PublicKey,
    pub access_key: EncryptedUserAccessKey,
}

impl UserAccessInfo {
    pub fn encrypt(
        account: &Account, pub_k: &PublicKey, key: &AESKey,
    ) -> SharedResult<HashMap<Username, Self>> {
        let priv_k = account.private_key;
        let user_key = pubkey::get_aes_key(&priv_k, pub_k)?;
        let encrypted_file_key = symkey::encrypt(&user_key, key)?;
        let mut result = HashMap::new();
        result.insert(
            account.username.clone(),
            UserAccessInfo {
                username: account.username.clone(),
                encrypted_by: *pub_k,
                access_key: encrypted_file_key,
            },
        );
        Ok(result)
    }
}
