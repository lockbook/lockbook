extern crate rand;

use self::rand::rngs::OsRng;
use self::rand::RngCore;
use crate::crypto::*;
use crate::{SharedError, SharedResult};
use aead::{generic_array::GenericArray, Aead, NewAead};
use aes_gcm::Aes256Gcm;
use hmac::Hmac;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::Sha256;

pub type HmacSha256 = Hmac<Sha256>;

pub fn generate_key() -> AESKey {
    let mut random_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut random_bytes);
    random_bytes
}

pub fn encrypt<T: Serialize + DeserializeOwned>(
    key: &AESKey, to_encrypt: &T,
) -> SharedResult<AESEncrypted<T>> {
    let serialized = bincode::serialize(to_encrypt).map_err(SharedError::Serialization)?;
    let nonce = &generate_nonce();
    let encrypted = convert_key(key)
        .encrypt(GenericArray::from_slice(nonce), aead::Payload { msg: &serialized, aad: &[] })
        .map_err(SharedError::Encryption)?;
    Ok(AESEncrypted::new(encrypted, nonce.to_vec()))
}
pub fn decrypt<T: DeserializeOwned>(key: &AESKey, to_decrypt: &AESEncrypted<T>) -> SharedResult<T> {
    let nonce = GenericArray::from_slice(&to_decrypt.nonce);
    let decrypted = convert_key(key)
        .decrypt(nonce, aead::Payload { msg: &to_decrypt.value, aad: &[] })
        .map_err(SharedError::Decryption)?;
    let deserialized = bincode::deserialize(&decrypted).map_err(SharedError::Deserialization)?;
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

    use crate::symkey::{decrypt, encrypt, generate_key};

    #[test]
    fn test_key_generation() {
        let key = generate_key();
        let test_value = Uuid::new_v4().to_string();
        let encrypted = encrypt(&key, &test_value).unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(test_value, decrypted)
    }

    #[test]
    fn test_hmac_encryption() {
        let key = generate_key();
        let test_value = Uuid::new_v4().to_string();
        let enc_hash = encrypt_and_hmac(&key, &test_value).unwrap();
        let decrypted = decrypt_and_verify(&key, &enc_hash).unwrap();

        assert_eq!(test_value, decrypted);
    }

    #[test]
    fn test_hmac_encryption_failure() {
        let key = generate_key();
        let test_value = Uuid::new_v4().to_string();
        let mut enc_hash = encrypt_and_hmac(&key, &test_value).unwrap();
        enc_hash.hmac[10] = 0;
        enc_hash.hmac[11] = 0;
        enc_hash.hmac[12] = 0;
        enc_hash.hmac[13] = 0;
        enc_hash.hmac[14] = 0;
        decrypt_and_verify(&key, &enc_hash).unwrap_err();
    }

    #[test]
    fn attempt_value_forge() {
        let key = generate_key();
        let test_value1 = Uuid::new_v4().to_string();
        let test_value2 = Uuid::new_v4().to_string();
        let enc_hash1 = encrypt_and_hmac(&key, &test_value1).unwrap();
        let mut enc_hash2 = encrypt_and_hmac(&key, &test_value2).unwrap();

        enc_hash2.encrypted_value = enc_hash1.encrypted_value;

        decrypt_and_verify(&key, &enc_hash2).unwrap_err();
    }
}
