extern crate rand;
extern crate rsa;

use std::string::FromUtf8Error;

use aead::{generic_array::GenericArray, Aead, NewAead};
use aes_gcm::Aes256Gcm;
use sha2::{Digest, Sha256};

use self::rand::rngs::OsRng;
use self::rand::RngCore;
use self::rsa::hash::Hashes;
use self::rsa::{PaddingScheme, PublicKey, RSAPrivateKey, RSAPublicKey};
use lockbook_models::crypto::*;

#[derive(Debug)]
pub enum DecryptionFailed {
    ValueCorrupted(base64::DecodeError),
    DecryptionFailed(rsa::errors::Error),
    DecryptedValueMalformed(FromUtf8Error),
}

#[derive(Debug)]
pub enum SignatureVerificationFailed {
    SignatureCorrupted(base64::DecodeError),
    VerificationFailed(rsa::errors::Error),
}

pub trait PubKeyCryptoService {
    fn generate_key() -> Result<RSAPrivateKey, rsa::errors::Error>;
    fn encrypt(
        public_key: &RSAPublicKey,
        decrypted: &DecryptedValue,
    ) -> Result<EncryptedValue, rsa::errors::Error>;
    fn sign(private_key: &RSAPrivateKey, to_sign: &str) -> Result<SignedValue, rsa::errors::Error>;
    fn verify(
        public_key: &RSAPublicKey,
        signed_value: &SignedValue,
    ) -> Result<(), SignatureVerificationFailed>;
    fn decrypt(
        private_key: &RSAPrivateKey,
        encrypted: &EncryptedValue,
    ) -> Result<DecryptedValue, DecryptionFailed>;
}

pub struct RsaImpl;

impl PubKeyCryptoService for RsaImpl {
    fn generate_key() -> Result<RSAPrivateKey, rsa::errors::Error> {
        let mut rng = OsRng;
        let bits = 2048;

        RSAPrivateKey::new(&mut rng, bits)
    }

    fn encrypt(
        public_key: &RSAPublicKey,
        decrypted: &DecryptedValue,
    ) -> Result<EncryptedValue, rsa::errors::Error> {
        let mut rng = OsRng;
        let data_in = decrypted.secret.as_bytes();
        let encrypted_data = public_key.encrypt(&mut rng, PaddingScheme::PKCS1v15, &data_in)?;
        let encoded = base64::encode(&encrypted_data);

        Ok(EncryptedValue { garbage: encoded })
    }

    fn sign(private_key: &RSAPrivateKey, to_sign: &str) -> Result<SignedValue, rsa::errors::Error> {
        let digest = Sha256::digest(to_sign.as_bytes()).to_vec();
        let signature =
            private_key.sign(PaddingScheme::PKCS1v15, Some(&Hashes::SHA2_256), &digest)?;
        let encoded_signature = base64::encode(&signature);

        Ok(SignedValue {
            content: String::from(to_sign),
            signature: encoded_signature,
        })
    }

    fn verify(
        public_key: &RSAPublicKey,
        signed_value: &SignedValue,
    ) -> Result<(), SignatureVerificationFailed> {
        let digest = Sha256::digest(signed_value.content.as_bytes()).to_vec();
        let signature = base64::decode(&signed_value.signature)
            .map_err(SignatureVerificationFailed::SignatureCorrupted)?;

        Ok(public_key
            .verify(
                PaddingScheme::PKCS1v15,
                Some(&Hashes::SHA2_256),
                &digest,
                &signature,
            )
            .map_err(SignatureVerificationFailed::VerificationFailed)?)
    }

    fn decrypt(
        private_key: &RSAPrivateKey,
        encrypted: &EncryptedValue,
    ) -> Result<DecryptedValue, DecryptionFailed> {
        let data = base64::decode(&encrypted.garbage).map_err(DecryptionFailed::ValueCorrupted)?;
        let secret = private_key
            .decrypt(PaddingScheme::PKCS1v15, &data)
            .map_err(DecryptionFailed::DecryptionFailed)?;
        let string = String::from_utf8(secret.to_vec())
            .map_err(DecryptionFailed::DecryptedValueMalformed)?;

        Ok(DecryptedValue { secret: string })
    }
}

#[cfg(test)]
mod unit_test_pubkey {
    use crate::service::crypto_service::{DecryptedValue, PubKeyCryptoService, RsaImpl};

    use super::rsa::RSAPrivateKey;

    #[test]
    fn test_key_generation_serde() {
        let key = RsaImpl::generate_key().unwrap();

        let key_read: RSAPrivateKey =
            serde_json::from_str(serde_json::to_string(&key).unwrap().as_str()).unwrap();
        key_read
            .validate()
            .expect("Invalid key after serialize deserialize");
        assert_eq!(key, key_read)
    }

    #[test]
    fn test_sign_verify() {
        let key = RsaImpl::generate_key().unwrap();

        let value = RsaImpl::sign(&key, "Test").unwrap();
        assert_eq!(value.content, "Test");

        RsaImpl::verify(&key.to_public_key(), &value).unwrap();
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key = RsaImpl::generate_key().unwrap();

        let encrypted = RsaImpl::encrypt(
            &key.to_public_key(),
            &DecryptedValue {
                secret: "Secret".to_string(),
            },
        )
        .unwrap();
        let decrypted = RsaImpl::decrypt(&key, &encrypted).unwrap();

        assert_eq!(decrypted.secret, "Secret".to_string());
    }
}

#[derive(Debug)]
pub enum AesEncryptionFailed {
    KeyCorrupted(base64::DecodeError),
    EncryptionFailed(aead::Error),
}

#[derive(Debug)]
pub enum AesDecryptionFailed {
    DecryptionFailed(aead::Error),
    DecryptedValueMalformed(FromUtf8Error),
    ValueCorrupted(base64::DecodeError),
}

pub trait SymmetricCryptoService {
    fn generate_key() -> AesKey;
    fn encrypt(
        key: &AesKey,
        secret: &DecryptedValue,
    ) -> Result<EncryptedValueWithNonce, AesEncryptionFailed>;
    fn decrypt(
        key: &AesKey,
        encrypted: &EncryptedValueWithNonce,
    ) -> Result<DecryptedValue, AesDecryptionFailed>;
}

pub struct AesImpl;

impl SymmetricCryptoService for AesImpl {
    fn generate_key() -> AesKey {
        let mut random_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut random_bytes);

        AesKey {
            key: base64::encode(&random_bytes.to_vec()),
        }
    }

    fn encrypt(
        aes_key: &AesKey,
        secret: &DecryptedValue,
    ) -> Result<EncryptedValueWithNonce, AesEncryptionFailed> {
        let key_bytes = base64::decode(&aes_key.key).map_err(AesEncryptionFailed::KeyCorrupted)?;
        let key_bytes_array = GenericArray::clone_from_slice(&key_bytes);
        let key = Aes256Gcm::new(key_bytes_array);

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = GenericArray::from_slice(&nonce_bytes);

        let secret = secret.secret.as_bytes();
        let cipher_text = key
            .encrypt(&nonce, secret)
            .map_err(AesEncryptionFailed::EncryptionFailed)?;

        Ok(EncryptedValueWithNonce {
            garbage: base64::encode(&cipher_text),
            nonce: base64::encode(&nonce_bytes),
        })
    }

    fn decrypt(
        aes_key: &AesKey,
        encrypted: &EncryptedValueWithNonce,
    ) -> Result<DecryptedValue, AesDecryptionFailed> {
        let key_bytes = base64::decode(&aes_key.key).unwrap();
        let key_bytes_array = GenericArray::clone_from_slice(&key_bytes);
        let key = Aes256Gcm::new(key_bytes_array);

        let decoded_nonce =
            base64::decode(&encrypted.nonce).map_err(AesDecryptionFailed::ValueCorrupted)?;

        let nonce = GenericArray::clone_from_slice(&decoded_nonce);
        let ciphertext =
            base64::decode(&encrypted.garbage).map_err(AesDecryptionFailed::ValueCorrupted)?;
        let secret = key
            .decrypt(&nonce, ciphertext.as_ref())
            .map_err(AesDecryptionFailed::DecryptionFailed)?;
        let string = String::from_utf8(secret.to_vec())
            .map_err(AesDecryptionFailed::DecryptedValueMalformed)?;

        Ok(DecryptedValue { secret: string })
    }
}

#[cfg(test)]
mod unit_test_symmetric {
    use uuid::Uuid;

    use crate::service::crypto_service::{AesImpl, DecryptedValue, SymmetricCryptoService};

    #[test]
    fn test_key_generation() {
        let key = AesImpl::generate_key();
        let test_value = Uuid::new_v4().to_string();
        let encrypted = AesImpl::encrypt(
            &key,
            &DecryptedValue {
                secret: test_value.clone(),
            },
        )
        .unwrap();
        let decrypted = AesImpl::decrypt(&key, &encrypted).unwrap();
        assert_eq!(test_value, decrypted.secret)
    }
}
