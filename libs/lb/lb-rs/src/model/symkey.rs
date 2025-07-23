use crate::model::crypto::*;
use aead::generic_array::GenericArray;
use aead::{Aead, NewAead};
use aes_gcm::Aes256Gcm;
use rand::RngCore;
use rand::rngs::OsRng;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::errors::{CryptoError, LbErrKind, LbResult, Unexpected};

pub fn generate_key() -> AESKey {
    let mut random_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut random_bytes);
    random_bytes
}

pub fn encrypt<T: Serialize + DeserializeOwned>(
    key: &AESKey, to_encrypt: &T,
) -> LbResult<AESEncrypted<T>> {
    let serialized = bincode::serialize(to_encrypt).map_unexpected()?;
    let nonce = &generate_nonce();
    let encrypted = convert_key(key)
        .encrypt(GenericArray::from_slice(nonce), aead::Payload { msg: &serialized, aad: &[] })
        .map_unexpected()?;
    Ok(AESEncrypted::new(encrypted, nonce.to_vec()))
}

pub fn decrypt<T: DeserializeOwned>(key: &AESKey, to_decrypt: &AESEncrypted<T>) -> LbResult<T> {
    let nonce = GenericArray::from_slice(&to_decrypt.nonce);
    let decrypted = convert_key(key)
        .decrypt(nonce, aead::Payload { msg: &to_decrypt.value, aad: &[] })
        .map_err(|err| LbErrKind::Crypto(CryptoError::Decryption(err)))?;
    let deserialized = bincode::deserialize(&decrypted).map_unexpected()?;
    Ok(deserialized)
}

pub fn convert_key(to_convert: &AESKey) -> Aes256Gcm {
    Aes256Gcm::new(&GenericArray::clone_from_slice(to_convert))
}

pub fn generate_nonce() -> [u8; 12] {
    let mut result = [0u8; 12];
    OsRng.fill_bytes(&mut result);
    result
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::symkey::{decrypt, encrypt, generate_key};

    #[test]
    fn test_generate_encrypt_decrypt() {
        let key = generate_key();
        let test_value = Uuid::new_v4().to_string();
        let encrypted = encrypt(&key, &test_value).unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(test_value, decrypted)
    }
}
