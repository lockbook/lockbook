extern crate rand;
extern crate rsa;

use aead::{generic_array::GenericArray, Aead, NewAead};
use aes_gcm::Aes256Gcm;
use sha2::{Digest, Sha256};

use crate::model::crypto::*;

use self::rand::rngs::OsRng;
use self::rand::RngCore;
use self::rsa::hash::Hashes;
use self::rsa::{PaddingScheme, PublicKey, RSAPrivateKey, RSAPublicKey};
use crate::service::clock_service::Clock;
use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Debug)]
pub enum RSAEncryptError {
    Serialization(bincode::Error),
    Encryption(rsa::errors::Error),
}

#[derive(Debug)]
pub enum RSADecryptError {
    Decryption(rsa::errors::Error),
    Deserialization(bincode::Error),
}

#[derive(Debug)]
pub enum RSASignError {
    Serialization(bincode::Error),
    Signing(rsa::errors::Error),
}

#[derive(Debug)]
pub enum RSAVerifyError {
    Serialization(bincode::Error),
    Verification(rsa::errors::Error),
    WrongPublicKey,
    SignatureExpired(u64),
    SignatureInTheFuture(u64),
}

pub trait PubKeyCryptoService {
    fn generate_key() -> Result<RSAPrivateKey, rsa::errors::Error>;
    fn encrypt<T: Serialize + DeserializeOwned>(
        public_key: &RSAPublicKey,
        to_encrypt: &T,
    ) -> Result<RSAEncrypted<T>, RSAEncryptError>;
    fn decrypt<T: DeserializeOwned>(
        private_key: &RSAPrivateKey,
        to_decrypt: &RSAEncrypted<T>,
    ) -> Result<T, RSADecryptError>;
    fn sign<T: Serialize>(
        private_key: &RSAPrivateKey,
        to_sign: T,
    ) -> Result<RSASigned<T>, RSASignError>;
    fn verify<T: Serialize>(
        public_key: &RSAPublicKey,
        to_verify: &RSASigned<T>,
        max_delay_ms: u64,
        max_skew_ms: u64,
    ) -> Result<(), RSAVerifyError>;
}

pub struct RSAImpl<Time: Clock> {
    _clock: Time,
}

impl<Time: Clock> PubKeyCryptoService for RSAImpl<Time> {
    fn generate_key() -> Result<RSAPrivateKey, rsa::errors::Error> {
        RSAPrivateKey::new(&mut OsRng, 2048)
    }

    fn encrypt<T: Serialize + DeserializeOwned>(
        public_key: &RSAPublicKey,
        to_encrypt: &T,
    ) -> Result<RSAEncrypted<T>, RSAEncryptError> {
        let serialized = bincode::serialize(to_encrypt).map_err(RSAEncryptError::Serialization)?;
        let encrypted = public_key
            .encrypt(&mut OsRng, PaddingScheme::PKCS1v15, &serialized)
            .map_err(RSAEncryptError::Encryption)?;
        Ok(RSAEncrypted::new(encrypted))
    }

    fn decrypt<T: DeserializeOwned>(
        private_key: &RSAPrivateKey,
        to_decrypt: &RSAEncrypted<T>,
    ) -> Result<T, RSADecryptError> {
        let decrypted = private_key
            .decrypt(PaddingScheme::PKCS1v15, &to_decrypt.value)
            .map_err(RSADecryptError::Decryption)?;
        let deserialized =
            bincode::deserialize(&decrypted).map_err(RSADecryptError::Deserialization)?;
        Ok(deserialized)
    }

    fn sign<T: Serialize>(
        private_key: &RSAPrivateKey,
        to_sign: T,
    ) -> Result<RSASigned<T>, RSASignError> {
        let timestamped = Time::timestamp(to_sign);
        let serialized = bincode::serialize(&timestamped).map_err(RSASignError::Serialization)?;
        let digest = Sha256::digest(&serialized).to_vec();
        let signature = private_key
            .sign(PaddingScheme::PKCS1v15, Some(&Hashes::SHA2_256), &digest)
            .map_err(RSASignError::Signing)?;
        Ok(RSASigned {
            timestamped_value: timestamped,
            signature: signature,
            public_key: private_key.to_public_key(),
        })
    }

    fn verify<T: Serialize>(
        public_key: &RSAPublicKey,
        to_verify: &RSASigned<T>,
        max_delay_ms: u64,
        max_skew_ms: u64,
    ) -> Result<(), RSAVerifyError> {
        if public_key != &to_verify.public_key {
            return Err(RSAVerifyError::WrongPublicKey);
        }

        let auth_time = to_verify.timestamped_value.timestamp;
        let current_time = Time::get_time();
        let max_delay_ms = max_delay_ms as i64;
        let max_skew_ms = max_skew_ms as i64;
        if current_time < auth_time - max_skew_ms {
            return Err(RSAVerifyError::SignatureInTheFuture(
                (current_time - (auth_time - max_delay_ms)) as u64,
            ));
        }
        if current_time > auth_time + max_delay_ms {
            return Err(RSAVerifyError::SignatureExpired(
                (auth_time + max_delay_ms - current_time) as u64,
            ));
        }

        let serialized = bincode::serialize(&to_verify.timestamped_value)
            .map_err(RSAVerifyError::Serialization)?;
        let digest = Sha256::digest(&serialized).to_vec();
        to_verify
            .public_key
            .verify(
                PaddingScheme::PKCS1v15,
                Some(&Hashes::SHA2_256),
                &digest,
                &to_verify.signature,
            )
            .map_err(RSAVerifyError::Verification)?;

        Ok(())
    }
}

#[cfg(test)]
mod unit_test_pubkey {
    use super::rsa::RSAPrivateKey;
    use crate::service::clock_service::Clock;
    use crate::service::crypto_service::{PubKeyCryptoService, RSAImpl};

    struct EarlyClock;
    impl Clock for EarlyClock {
        fn get_time() -> i64 {
            500
        }
    }

    struct LateClock;
    impl Clock for LateClock {
        fn get_time() -> i64 {
            520
        }
    }

    #[test]
    fn test_key_generation_serde() {
        let key = RSAImpl::<EarlyClock>::generate_key().unwrap();

        let key_read: RSAPrivateKey =
            serde_json::from_str(serde_json::to_string(&key).unwrap().as_str()).unwrap();
        key_read
            .validate()
            .expect("Invalid key after serialize deserialize");
        assert_eq!(key, key_read)
    }

    #[test]
    fn test_sign_verify() {
        let key = RSAImpl::<EarlyClock>::generate_key().unwrap();
        let value = RSAImpl::<EarlyClock>::sign(&key, "Test").unwrap();
        RSAImpl::<LateClock>::verify(&key.to_public_key(), &value, 20, 20).unwrap();
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key = RSAImpl::<EarlyClock>::generate_key().unwrap();
        let encrypted =
            RSAImpl::<EarlyClock>::encrypt(&key.to_public_key(), &String::from("Secret")).unwrap();
        let decrypted = RSAImpl::<EarlyClock>::decrypt(&key, &encrypted).unwrap();
        assert_eq!(decrypted, "Secret");
    }
}

#[derive(Debug)]
pub enum AESEncryptError {
    Serialization(bincode::Error),
    Encryption(aead::Error),
}

#[derive(Debug)]
pub enum AESDecryptError {
    Decryption(aead::Error),
    Deserialization(bincode::Error),
}

pub trait SymmetricCryptoService {
    fn generate_key() -> AESKey;
    fn encrypt<T: Serialize + DeserializeOwned>(
        key: &AESKey,
        to_encrypt: &T,
    ) -> Result<AESEncrypted<T>, AESEncryptError>;
    fn decrypt<T: DeserializeOwned>(
        key: &AESKey,
        to_decrypt: &AESEncrypted<T>,
    ) -> Result<T, AESDecryptError>;
}

pub struct AESImpl;

impl AESImpl {
    fn convert_key(to_convert: &AESKey) -> Aes256Gcm {
        Aes256Gcm::new(GenericArray::clone_from_slice(to_convert))
    }

    fn generate_nonce() -> [u8; 12] {
        let mut result = [0u8; 12];
        OsRng.fill_bytes(&mut result);
        result
    }
}

impl SymmetricCryptoService for AESImpl {
    fn generate_key() -> AESKey {
        let mut random_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut random_bytes);
        random_bytes
    }

    fn encrypt<T: Serialize + DeserializeOwned>(
        key: &AESKey,
        to_encrypt: &T,
    ) -> Result<AESEncrypted<T>, AESEncryptError> {
        let serialized = bincode::serialize(to_encrypt).map_err(AESEncryptError::Serialization)?;
        let nonce = &AESImpl::generate_nonce();
        let encrypted = AESImpl::convert_key(key)
            .encrypt(
                &GenericArray::from_slice(nonce),
                aead::Payload {
                    msg: &serialized,
                    aad: &[],
                },
            )
            .map_err(AESEncryptError::Encryption)?;
        Ok(AESEncrypted::new(encrypted, nonce.to_vec()))
    }

    fn decrypt<T: DeserializeOwned>(
        key: &AESKey,
        to_decrypt: &AESEncrypted<T>,
    ) -> Result<T, AESDecryptError> {
        let nonce = GenericArray::from_slice(&to_decrypt.nonce);
        let decrypted = AESImpl::convert_key(key)
            .decrypt(
                &nonce,
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
}

#[cfg(test)]
mod unit_test_symmetric {
    use uuid::Uuid;

    use crate::service::crypto_service::{AESImpl, SymmetricCryptoService};

    #[test]
    fn test_key_generation() {
        let key = AESImpl::generate_key();
        let test_value = Uuid::new_v4().to_string();
        let encrypted = AESImpl::encrypt(&key, &test_value).unwrap();
        let decrypted = AESImpl::decrypt(&key, &encrypted).unwrap();
        assert_eq!(test_value, decrypted)
    }
}
