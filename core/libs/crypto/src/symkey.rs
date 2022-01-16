extern crate rand;

use aead::{generic_array::GenericArray, Aead, NewAead};
use aes_gcm::Aes256Gcm;
use hmac::crypto_mac::{InvalidKeyLength, MacError};
use hmac::{Hmac, Mac, NewMac};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::Sha256;

use lockbook_models::crypto::*;

use self::rand::rngs::OsRng;
use self::rand::RngCore;

type HmacSha256 = Hmac<Sha256>;

pub fn generate_key() -> AESKey {
    let mut random_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut random_bytes);
    random_bytes
}

#[derive(Debug)]
pub enum AESEncryptError {
    Serialization(bincode::Error),
    Encryption(aead::Error),
}

pub fn encrypt<T: Serialize + DeserializeOwned>(
    key: &AESKey,
    to_encrypt: &T,
) -> Result<AESEncrypted<T>, AESEncryptError> {
    let serialized = bincode::serialize(to_encrypt).map_err(AESEncryptError::Serialization)?;
    let nonce = &generate_nonce();
    let encrypted = convert_key(key)
        .encrypt(
            GenericArray::from_slice(nonce),
            aead::Payload {
                msg: &serialized,
                aad: &[],
            },
        )
        .map_err(AESEncryptError::Encryption)?;
    Ok(AESEncrypted::new(encrypted, nonce.to_vec()))
}

#[derive(Debug)]
pub enum EncryptAndHmacError {
    HmacCreationError(InvalidKeyLength),
    Serialization(bincode::Error),
    Encryption(aead::Error),
}

pub fn encrypt_and_hmac(
    key: &AESKey,
    to_encrypt: &str,
) -> Result<SecretFileName, EncryptAndHmacError> {
    let serialized = bincode::serialize(to_encrypt).map_err(EncryptAndHmacError::Serialization)?;

    let hmac = {
        let mut mac =
            HmacSha256::new_from_slice(key).map_err(EncryptAndHmacError::HmacCreationError)?;
        mac.update(serialized.as_ref());
        mac.finalize().into_bytes()
    }
    .into();

    let encrypted_value = {
        let nonce = &generate_nonce();
        let encrypted = convert_key(key)
            .encrypt(
                GenericArray::from_slice(nonce),
                aead::Payload {
                    msg: &serialized,
                    aad: &[],
                },
            )
            .map_err(EncryptAndHmacError::Encryption)?;
        AESEncrypted::new(encrypted, nonce.to_vec())
    };

    Ok(SecretFileName {
        encrypted_value,
        hmac,
    })
}

#[derive(Debug)]
pub enum AESDecryptError {
    Decryption(aead::Error),
    Deserialization(bincode::Error),
}

pub fn decrypt<T: DeserializeOwned>(
    key: &AESKey,
    to_decrypt: &AESEncrypted<T>,
) -> Result<T, AESDecryptError> {
    let nonce = GenericArray::from_slice(&to_decrypt.nonce);
    let decrypted = convert_key(key)
        .decrypt(
            nonce,
            aead::Payload {
                msg: &to_decrypt.value,
                aad: &[],
            },
        )
        .map_err(AESDecryptError::Decryption)?;
    let deserialized =
        bincode::deserialize(&decrypted).map_err(AESDecryptError::Deserialization)?;
    Ok(deserialized)
}

#[derive(Debug)]
pub enum DecryptAndVerifyError {
    Decryption(aead::Error),
    HmacCreationError(InvalidKeyLength),
    Deserialization(bincode::Error),
    HmacValidationError(MacError),
}

pub fn decrypt_and_verify(
    key: &AESKey,
    to_decrypt: &SecretFileName,
) -> Result<String, DecryptAndVerifyError> {
    let nonce = GenericArray::from_slice(&to_decrypt.encrypted_value.nonce);
    let decrypted = convert_key(key)
        .decrypt(
            nonce,
            aead::Payload {
                msg: &to_decrypt.encrypted_value.value,
                aad: &[],
            },
        )
        .map_err(DecryptAndVerifyError::Decryption)?;
    let deserialized =
        bincode::deserialize(&decrypted).map_err(DecryptAndVerifyError::Deserialization)?;

    let mut mac =
        HmacSha256::new_from_slice(key).map_err(DecryptAndVerifyError::HmacCreationError)?;
    mac.update(decrypted.as_ref());
    mac.verify(&to_decrypt.hmac)
        .map_err(DecryptAndVerifyError::HmacValidationError)?;

    Ok(deserialized)
}

fn convert_key(to_convert: &AESKey) -> Aes256Gcm {
    Aes256Gcm::new(&GenericArray::clone_from_slice(to_convert))
}

fn generate_nonce() -> [u8; 12] {
    let mut result = [0u8; 12];
    OsRng.fill_bytes(&mut result);
    result
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::symkey::{decrypt, decrypt_and_verify, encrypt, encrypt_and_hmac, generate_key};

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
