use crate::logic::crypto::*;
use crate::logic::{SharedErrorKind, SharedResult};
use aead::{generic_array::GenericArray, Aead, NewAead};
use aes_gcm::Aes256Gcm;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn generate_key() -> AESKey {
    let mut random_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut random_bytes);
    random_bytes
}

pub fn encrypt<T: Serialize + DeserializeOwned>(
    key: &AESKey, to_encrypt: &T,
) -> SharedResult<AESEncrypted<T>> {
    let serialized = bincode::serialize(to_encrypt)?;
    let nonce = &generate_nonce();
    let encrypted = convert_key(key)
        .encrypt(GenericArray::from_slice(nonce), aead::Payload { msg: &serialized, aad: &[] })
        .map_err(SharedErrorKind::Encryption)?;
    Ok(AESEncrypted::new(encrypted, nonce.to_vec()))
}
pub fn decrypt<T: DeserializeOwned>(key: &AESKey, to_decrypt: &AESEncrypted<T>) -> SharedResult<T> {
    let nonce = GenericArray::from_slice(&to_decrypt.nonce);
    let decrypted = convert_key(key)
        .decrypt(nonce, aead::Payload { msg: &to_decrypt.value, aad: &[] })
        .map_err(SharedErrorKind::Decryption)?;
    let deserialized = bincode::deserialize(&decrypted)?;
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

    use crate::logic::symkey::{decrypt, encrypt, generate_key};

    #[test]
    fn test_generate_encrypt_decrypt() {
        let key = generate_key();
        let test_value = Uuid::new_v4().to_string();
        let encrypted = encrypt(&key, &test_value).unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(test_value, decrypted)
    }
}
