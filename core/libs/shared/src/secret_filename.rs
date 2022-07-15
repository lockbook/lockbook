use crate::crypto::{AESEncrypted, AESKey};
use crate::symkey::{convert_key, generate_nonce, HmacSha256};
use crate::{SharedError, SharedResult};
use aead::{generic_array::GenericArray, Aead};
use hmac::{Mac, NewMac};
use serde::{Deserialize, Serialize};
use std::hash::Hash;

/// A secret value that can impl an equality check by hmac'ing the
/// inner secret.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SecretFileName {
    pub encrypted_value: AESEncrypted<String>,
    pub hmac: [u8; 32],
}

impl SecretFileName {
    pub fn from_str(to_encrypt: &str, key: &AESKey) -> SharedResult<Self> {
        let serialized = bincode::serialize(to_encrypt).map_err(SharedError::Serialization)?;

        let hmac = {
            let mut mac =
                HmacSha256::new_from_slice(key).map_err(SharedError::HmacCreationError)?;
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
                .map_err(SharedError::Encryption)?;
            AESEncrypted::new(encrypted, nonce.to_vec())
        };

        Ok(SecretFileName { encrypted_value, hmac })
    }

    pub fn to_string(&self, key: &AESKey) -> SharedResult<String> {
        let nonce = GenericArray::from_slice(&self.encrypted_value.nonce);
        let decrypted = convert_key(key)
            .decrypt(nonce, aead::Payload { msg: &self.encrypted_value.value, aad: &[] })
            .map_err(SharedError::Decryption)?;
        let deserialized =
            bincode::deserialize(&decrypted).map_err(SharedError::Deserialization)?;

        let mut mac = HmacSha256::new_from_slice(key).map_err(SharedError::HmacCreationError)?;
        mac.update(decrypted.as_ref());
        mac.verify(&self.hmac)
            .map_err(SharedError::HmacValidationError)?;

        Ok(deserialized)
    }
}

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
