use crate::model::crypto::{AESEncrypted, AESKey};
use crate::model::symkey::{convert_key, generate_nonce};
use aead::Aead;
use aead::generic_array::GenericArray;
use hmac::{Hmac, Mac, NewMac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::hash::Hash;

use super::errors::{CryptoError, LbErrKind, LbResult, Unexpected};

pub type HmacSha256 = Hmac<Sha256>;

/// A secret value that can impl an equality check by hmac'ing the
/// inner secret.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SecretFileName {
    pub encrypted_value: AESEncrypted<String>,
    pub hmac: [u8; 32],
}

impl SecretFileName {
    pub fn from_str(to_encrypt: &str, key: &AESKey, parent_key: &AESKey) -> LbResult<Self> {
        let serialized = bincode::serialize(to_encrypt).map_unexpected()?;

        let hmac = {
            let mut mac = HmacSha256::new_from_slice(parent_key).map_unexpected()?;
            mac.update(serialized.as_ref());
            mac.finalize().into_bytes()
        }
        .into();

        let encrypted_value = {
            let nonce = &generate_nonce();
            let encrypted = convert_key(key)
                .encrypt(
                    GenericArray::from_slice(nonce),
                    aead::Payload { msg: &serialized, aad: &[] },
                )
                .map_unexpected()?;
            AESEncrypted::new(encrypted, nonce.to_vec())
        };

        Ok(SecretFileName { encrypted_value, hmac })
    }

    pub fn to_string(&self, key: &AESKey) -> LbResult<String> {
        let nonce = GenericArray::from_slice(&self.encrypted_value.nonce);
        let decrypted = convert_key(key)
            .decrypt(nonce, aead::Payload { msg: &self.encrypted_value.value, aad: &[] })
            .map_err(|err| LbErrKind::Crypto(CryptoError::Decryption(err)))?;
        let deserialized = bincode::deserialize(&decrypted).map_unexpected()?;
        Ok(deserialized)
    }

    pub fn verify_hmac(&self, key: &AESKey, parent_key: &AESKey) -> LbResult<()> {
        let decrypted = self.to_string(key)?;
        let mut mac = HmacSha256::new_from_slice(parent_key).map_unexpected()?;
        mac.update(decrypted.as_ref());
        mac.verify(&self.hmac)
            .map_err(|err| LbErrKind::Crypto(CryptoError::HmacVerification(err)))?;
        Ok(())
    }
}

// Impl'd to avoid comparing encrypted values
impl PartialEq for SecretFileName {
    fn eq(&self, other: &Self) -> bool {
        self.hmac == other.hmac
    }
}

impl Eq for SecretFileName {}

impl Hash for SecretFileName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hmac.hash(state);
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::secret_filename::SecretFileName;
    use crate::model::symkey::generate_key;
    use uuid::Uuid;

    #[test]
    fn test_to_string_from_string() {
        let key = generate_key();
        let parent_key = generate_key();
        let test_value = Uuid::new_v4().to_string();
        let secret = SecretFileName::from_str(&test_value, &key, &parent_key).unwrap();
        let decrypted = secret.to_string(&key).unwrap();

        assert_eq!(test_value, decrypted);
    }

    #[test]
    fn test_hmac_encryption_failure() {
        let key = generate_key();
        let parent_key = generate_key();
        let test_value = Uuid::new_v4().to_string();
        let mut secret = SecretFileName::from_str(&test_value, &key, &parent_key).unwrap();
        secret.hmac[10] = !secret.hmac[10];
        secret.hmac[11] = !secret.hmac[11];
        secret.hmac[12] = !secret.hmac[12];
        secret.hmac[13] = !secret.hmac[13];
        secret.hmac[14] = !secret.hmac[14];
        secret.verify_hmac(&key, &parent_key).unwrap_err();
    }

    #[test]
    fn attempt_value_forge() {
        let key = generate_key();
        let parent_key = generate_key();

        let test_value1 = Uuid::new_v4().to_string();
        let test_value2 = Uuid::new_v4().to_string();
        let secret1 = SecretFileName::from_str(&test_value1, &key, &parent_key).unwrap();
        let mut secret2 = SecretFileName::from_str(&test_value2, &key, &parent_key).unwrap();

        secret2.encrypted_value = secret1.encrypted_value;

        secret2.verify_hmac(&key, &parent_key).unwrap_err();
    }
}
