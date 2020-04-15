extern crate rand;
extern crate rsa;

use sha2::{Digest, Sha256};

use crate::error_enum;

use self::rand::rngs::OsRng;
use self::rsa::hash::Hashes;
use self::rsa::{PaddingScheme, PublicKey, RSAPrivateKey, RSAPublicKey};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug)]
pub struct EncryptedValue {
    pub garbage: String,
}

#[derive(PartialEq, Debug)]
pub struct DecryptedValue {
    pub secret: String,
}

#[derive(Serialize, Deserialize)]
pub struct SignedValue {
    pub content: String,
    pub signature: String,
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
}