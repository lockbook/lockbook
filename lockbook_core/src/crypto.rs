extern crate base64;
extern crate openssl;

use std::ops::Try;
use std::option::NoneError;

use base64::{decode, encode};
use openssl::bn::BigNum;
use openssl::rsa::Rsa;

use crate::error_enum;

use self::openssl::error::ErrorStack;
use self::openssl::pkey::Private;
use self::openssl::rsa::Padding;
use self::openssl::symm::Mode::Encrypt;

#[derive(PartialEq, Debug)]
pub struct PublicKey {
    pub n: String,
    pub e: String,
}

#[derive(PartialEq, Debug)]
pub struct PrivateKey {
    pub d: String,
    pub p: String,
    pub q: String,
    pub dmp1: String,
    pub dmq1: String,
    pub iqmp: String,
}

#[derive(PartialEq, Debug)]
pub struct KeyPair {
    pub public_key: PublicKey,
    pub private_key: PrivateKey,
}

error_enum! {
    enum BigNumError {
        NotBase64(base64::DecodeError),
        NotBigNumber(ErrorStack),
    }
}

error_enum! {
    enum DecodingError {
        DecodingError(base64::DecodeError),
        KeyBuildFailed(ErrorStack),
    }
}

error_enum! {
    enum KeyGenError {
        KeyGenerationError(ErrorStack),
        KeyComponentMissing(NoneError),
    }
}

error_enum! {
    enum EncryptionError {
        KeyMalformed(DecodingError),
    }
}

pub enum DecryptionError {}

impl KeyPair {
    fn get_big_num(s: &String) -> Result<BigNum, DecodingError> {
        Ok(
            BigNum::from_slice(
                &decode(&s)?
            )?
        )
    }

    fn get_openssl_key(&self) -> Result<Rsa<Private>, DecodingError> {
        Ok(
            Rsa::from_private_components(
                KeyPair::get_big_num(&self.public_key.n)?,
                KeyPair::get_big_num(&self.public_key.e)?,
                KeyPair::get_big_num(&self.private_key.d)?,
                KeyPair::get_big_num(&self.private_key.p)?,
                KeyPair::get_big_num(&self.private_key.q)?,
                KeyPair::get_big_num(&self.private_key.dmp1)?,
                KeyPair::get_big_num(&self.private_key.dmq1)?,
                KeyPair::get_big_num(&self.private_key.iqmp)?,
            )?
        )
    }
}

pub struct EncryptedValue {
    pub garbage: String
}

pub struct DecryptedValue {
    pub secret: String
}

pub trait CryptoService {
    fn generate_key() -> Result<KeyPair, KeyGenError>;
    fn verify_key(key: &KeyPair) -> Result<bool, DecodingError>;

    fn encrypt_public(key: &KeyPair, decrypted: &DecryptedValue) -> Result<EncryptedValue, EncryptionError>;
    fn decrypt_public(key: &KeyPair, encrypted: &EncryptedValue) -> Result<EncryptionError, DecryptedValue>;

    fn encrypt_private(key: &KeyPair, decrypted: &DecryptedValue) -> Result<EncryptedValue, EncryptionError>;
    fn decrypt_private(key: &KeyPair, encrypted: &EncryptedValue) -> Result<EncryptionError, DecryptedValue>;
}

pub struct RsaCryptoService;

impl CryptoService for RsaCryptoService {
    fn generate_key() -> Result<KeyPair, KeyGenError> {
        let their_key = Rsa::generate(2048)?;

        Ok(
            KeyPair {
                public_key: PublicKey {
                    n: encode(&their_key.n().to_vec()),
                    e: encode(&their_key.e().to_vec()),
                },
                private_key: PrivateKey {
                    d: encode(&their_key.d().to_vec()),
                    p: encode(&their_key.p().into_result()?.to_vec()),
                    q: encode(&their_key.q().into_result()?.to_vec()),
                    dmp1: encode(&their_key.dmp1().into_result()?.to_vec()),
                    dmq1: encode(&their_key.dmq1().into_result()?.to_vec()),
                    iqmp: encode(&their_key.iqmp().into_result()?.to_vec()),
                },
            }
        )
    }

    fn verify_key(keypair: &KeyPair) -> Result<bool, DecodingError> {
        Ok(
            keypair
                .get_openssl_key()?
                .check_key()?
        )
    }

    fn encrypt_public(key: &KeyPair, decrypted: &DecryptedValue) -> Result<EncryptedValue, EncryptionError> {
        let openssl_key = key.get_openssl_key()?;
        let data_in = decrypted.secret.as_bytes();
        let mut data_out = vec![0; openssl_key.size() as usize];
        let encrypted_len = openssl_key.public_encrypt(data_in, &mut data_out, Padding::PKCS1);
        let encoded = encode(&data_out);
        Ok(EncryptedValue { garbage: encoded })
    }

    fn decrypt_public(key: &KeyPair, encrypted: &EncryptedValue) -> Result<EncryptionError, DecryptedValue> {
        unimplemented!()
    }

    fn encrypt_private(key: &KeyPair, decrypted: &DecryptedValue) -> Result<EncryptedValue, EncryptionError> {
        let openssl_key = key.get_openssl_key()?;
        let data_in = decrypted.secret.as_bytes();
        let mut data_out = vec![0; openssl_key.size() as usize];
        let encrypted_len = openssl_key.private_encrypt(data_in, &mut data_out, Padding::PKCS1);
        let encoded = encode(&data_out);
        Ok(EncryptedValue { garbage: encoded })
    }

    fn decrypt_private(key: &KeyPair, encrypted: &EncryptedValue) -> Result<EncryptionError, DecryptedValue> {
        unimplemented!()
    }
}

#[cfg(test)]
mod unit_test {
    use crate::crypto::{CryptoService, RsaCryptoService};

    #[test]
    fn test_key_generation() {
        let key = RsaCryptoService::generate_key().unwrap();
        assert!(RsaCryptoService::verify_key(&key).unwrap());
    }
}

