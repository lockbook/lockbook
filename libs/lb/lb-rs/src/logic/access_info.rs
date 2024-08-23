use crate::logic::account::Account;
use crate::logic::crypto::{AESEncrypted, AESKey};
use crate::logic::{pubkey, symkey, SharedResult};
use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};

pub type EncryptedUserAccessKey = AESEncrypted<AESKey>;
pub type EncryptedFolderAccessKey = AESEncrypted<AESKey>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd, Ord)]
pub enum UserAccessMode {
    Read,
    Write,
    Owner, // todo: remove
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
        self.mode == other.mode
            && self.encrypted_for == other.encrypted_for
            && self.encrypted_by == other.encrypted_by
            && self.deleted == other.deleted
    }
}

impl UserAccessInfo {
    pub fn encrypt(
        account: &Account, encrypted_by: &PublicKey, encrypted_for: &PublicKey, key: &AESKey,
        mode: UserAccessMode,
    ) -> SharedResult<Self> {
        let private_key = account.private_key;
        let user_key = pubkey::get_aes_key(&private_key, encrypted_for)?;
        let encrypted_file_key = symkey::encrypt(&user_key, key)?;
        Ok(UserAccessInfo {
            mode,
            encrypted_by: *encrypted_by,
            encrypted_for: *encrypted_for,
            access_key: encrypted_file_key,
            deleted: false,
        })
    }

    pub fn decrypt(&self, account: &Account) -> SharedResult<AESKey> {
        let shared_secret = pubkey::get_aes_key(&account.private_key, &self.encrypted_by)?;
        let encrypted = &self.access_key;
        let decrypted = symkey::decrypt(&shared_secret, encrypted)?;
        Ok(decrypted)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::logic::access_info::{UserAccessInfo, UserAccessMode};
    use crate::logic::account::Account;
    use crate::logic::symkey;

    #[test]
    fn encrypt_decrypt_1() {
        let account = Account::new("test1".to_string(), "test2".to_string());
        let key = symkey::generate_key();
        let encrypted = UserAccessInfo::encrypt(
            &account,
            &account.public_key(),
            &account.public_key(),
            &key,
            UserAccessMode::Write,
        )
        .unwrap();
        let decrypted = encrypted.decrypt(&account).unwrap();
        assert_eq!(key, decrypted);
    }

    #[test]
    fn encrypt_decrypt_2() {
        let account1 = Account::new("test1".to_string(), "test2".to_string());
        let account2 = Account::new("test2".to_string(), "test2".to_string());
        let key = symkey::generate_key();
        let encrypted = UserAccessInfo::encrypt(
            &account1,
            &account1.public_key(),
            &account2.public_key(),
            &key,
            UserAccessMode::Write,
        )
        .unwrap();
        let decrypted = encrypted.decrypt(&account2).unwrap();
        assert_eq!(key, decrypted);
    }
}
