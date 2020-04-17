extern crate rand;
extern crate rsa;

use std::string::FromUtf8Error;

use sha2::{Digest, Sha256};

use crate::error_enum;

use self::rand::rngs::OsRng;
use self::rsa::hash::Hashes;
use self::rsa::{PaddingScheme, PublicKey, RSAPrivateKey, RSAPublicKey};

#[derive(PartialEq, Debug)]
pub struct EncryptedValue {
    pub garbage: String,
}

#[derive(PartialEq, Debug)]
pub struct DecryptedValue {
    pub secret: String,
}

pub struct SignedValue {
    pub content: String,
    pub signature: String,
}

error_enum! {
    enum DecryptionFailed {
        ValueCorrupted(base64::DecodeError),
        DecryptionFailed(rsa::errors::Error),
        DecryptedValueMalformed(FromUtf8Error),
    }
}

error_enum! {
    enum SignatureVerificationFailed {
        SignatureCorrupted(base64::DecodeError),
        VerificationFailed(rsa::errors::Error),
    }
}

pub trait PubKeyCryptoService {
    fn generate_key() -> Result<RSAPrivateKey, rsa::errors::Error>;
    fn encrypt(
        public_key: &RSAPublicKey,
        decrypted: &DecryptedValue,
    ) -> Result<EncryptedValue, rsa::errors::Error>;
    fn sign(
        private_key: &RSAPrivateKey,
        to_sign: String,
    ) -> Result<SignedValue, rsa::errors::Error>;
    fn verify(
        public_key: &RSAPublicKey,
        signed_value: &SignedValue,
    ) -> Result<(), SignatureVerificationFailed>;
    fn decrypt(
        private_key: &RSAPrivateKey,
        encrypted: &EncryptedValue,
    ) -> Result<DecryptedValue, DecryptionFailed>;
}

pub struct RsaCryptoService;

impl PubKeyCryptoService for RsaCryptoService {
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

    fn sign(
        private_key: &RSAPrivateKey,
        to_sign: String,
    ) -> Result<SignedValue, rsa::errors::Error> {
        let digest = Sha256::digest(to_sign.as_bytes()).to_vec();
        let signature =
            private_key.sign(PaddingScheme::PKCS1v15, Some(&Hashes::SHA2_256), &digest)?;
        let encoded_signature = base64::encode(&signature);

        Ok(SignedValue {
            content: to_sign,
            signature: encoded_signature,
        })
    }

    fn verify(
        public_key: &RSAPublicKey,
        signed_value: &SignedValue,
    ) -> Result<(), SignatureVerificationFailed> {
        let digest = Sha256::digest(signed_value.content.as_bytes()).to_vec();
        let signature = base64::decode(&signed_value.signature)?;

        Ok(public_key.verify(
            PaddingScheme::PKCS1v15,
            Some(&Hashes::SHA2_256),
            &digest,
            &signature,
        )?)
    }

    fn decrypt(
        private_key: &RSAPrivateKey,
        encrypted: &EncryptedValue,
    ) -> Result<DecryptedValue, DecryptionFailed> {
        let data = base64::decode(&encrypted.garbage)?;
        let secret = private_key.decrypt(PaddingScheme::PKCS1v15, &data)?;
        let string = String::from_utf8(secret.to_vec())?;

        Ok(DecryptedValue { secret: string })
    }
}

#[cfg(test)]
mod unit_test {
    use crate::crypto::{DecryptedValue, PubKeyCryptoService, RsaCryptoService};

    use super::rsa::{PublicKey, RSAPrivateKey};

    #[test]
    fn test_key_generation_serde() {
        let key = RsaCryptoService::generate_key().unwrap();

        let key_read: RSAPrivateKey =
            serde_json::from_str(serde_json::to_string(&key).unwrap().as_str()).unwrap();
        key_read
            .validate()
            .expect("Invalid key after serialize deserialize");
        assert_eq!(key, key_read)
    }

    #[test]
    fn test_sign_verify() {
        let key = RsaCryptoService::generate_key().unwrap();

        let value = RsaCryptoService::sign(&key, "Test".to_string()).unwrap();
        assert_eq!(value.content, "Test");

        RsaCryptoService::verify(&key.to_public_key(), &value).unwrap();
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key = RsaCryptoService::generate_key().unwrap();

        let encrypted = RsaCryptoService::encrypt(
            &key.to_public_key(),
            &DecryptedValue {
                secret: "Secret".to_string(),
            },
        )
        .unwrap();
        let decrypted = RsaCryptoService::decrypt(&key, &encrypted).unwrap();

        assert_eq!(decrypted.secret, "Secret".to_string());
    }
}
