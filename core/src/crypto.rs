extern crate base64;
extern crate openssl;

use std::ops::Try;
use std::option::NoneError;
use std::string::FromUtf8Error;

use base64::{decode, encode};
use openssl::bn::BigNum;
use openssl::rsa::Rsa;

use crate::error_enum;

use self::openssl::error::ErrorStack;
use self::openssl::pkey::Private;
use self::openssl::rsa::Padding;

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
        InputTooLarge(usize),
        EncryptionFailed(ErrorStack),
    }
}

error_enum! {
    enum DecryptionError {
        KeyMalformed(DecodingError),
        EncryptedValueMalformed(base64::DecodeError),
        DecryptedValueMalformed(FromUtf8Error),
        EncryptionFailed(ErrorStack),
    }
}

impl KeyPair {
    fn get_big_num(s: &String) -> Result<BigNum, DecodingError> {
        Ok(BigNum::from_slice(&decode(&s)?)?)
    }

    fn get_openssl_key(&self) -> Result<Rsa<Private>, DecodingError> {
        Ok(Rsa::from_private_components(
            KeyPair::get_big_num(&self.public_key.n)?,
            KeyPair::get_big_num(&self.public_key.e)?,
            KeyPair::get_big_num(&self.private_key.d)?,
            KeyPair::get_big_num(&self.private_key.p)?,
            KeyPair::get_big_num(&self.private_key.q)?,
            KeyPair::get_big_num(&self.private_key.dmp1)?,
            KeyPair::get_big_num(&self.private_key.dmq1)?,
            KeyPair::get_big_num(&self.private_key.iqmp)?,
        )?)
    }
}

#[derive(PartialEq, Debug)]
pub struct EncryptedValue {
    pub garbage: String,
}

#[derive(PartialEq, Debug)]
pub struct DecryptedValue {
    pub secret: String,
}

pub trait CryptoService {
    fn generate_key() -> Result<KeyPair, KeyGenError>;
    fn verify_key(key: &KeyPair) -> Result<bool, DecodingError>;

    fn encrypt_public(
        key: &KeyPair,
        decrypted: &DecryptedValue,
    ) -> Result<EncryptedValue, EncryptionError>;
    fn decrypt_public(
        key: &KeyPair,
        encrypted: &EncryptedValue,
    ) -> Result<DecryptedValue, DecryptionError>;

    fn encrypt_private(
        key: &KeyPair,
        decrypted: &DecryptedValue,
    ) -> Result<EncryptedValue, EncryptionError>;
    fn decrypt_private(
        key: &KeyPair,
        encrypted: &EncryptedValue,
    ) -> Result<DecryptedValue, DecryptionError>;
}

pub struct RsaCryptoService;

impl CryptoService for RsaCryptoService {
    fn generate_key() -> Result<KeyPair, KeyGenError> {
        let their_key = Rsa::generate(2048)?;

        Ok(KeyPair {
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
        })
    }

    fn verify_key(keypair: &KeyPair) -> Result<bool, DecodingError> {
        Ok(keypair.get_openssl_key()?.check_key()?)
    }

    fn encrypt_public(
        key: &KeyPair,
        decrypted: &DecryptedValue,
    ) -> Result<EncryptedValue, EncryptionError> {
        let openssl_key = key.get_openssl_key()?;
        let data_in = decrypted.secret.as_bytes();
        let mut data_out = vec![0; openssl_key.size() as usize];
        let _encrypted_len = openssl_key.public_encrypt(data_in, &mut data_out, Padding::PKCS1)?;
        let encoded = encode(&data_out);

        Ok(EncryptedValue { garbage: encoded })
    }

    fn decrypt_public(
        key: &KeyPair,
        encrypted: &EncryptedValue,
    ) -> Result<DecryptedValue, DecryptionError> {
        let openssl_key = key.get_openssl_key()?;
        let data_in = decode(&encrypted.garbage)?;
        let mut data_out = vec![0; openssl_key.size() as usize];
        let decrypted_len = openssl_key.public_decrypt(&data_in, &mut data_out, Padding::PKCS1)?;
        let secret = String::from_utf8(data_out[0..decrypted_len].to_vec())?;

        Ok(DecryptedValue { secret })
    }

    fn encrypt_private(
        key: &KeyPair,
        decrypted: &DecryptedValue,
    ) -> Result<EncryptedValue, EncryptionError> {
        let openssl_key = key.get_openssl_key()?;
        let data_in = decrypted.secret.as_bytes();
        let mut data_out = vec![0; openssl_key.size() as usize];
        let _encrypted_len = openssl_key.private_encrypt(data_in, &mut data_out, Padding::PKCS1)?;
        let encoded = encode(&data_out);

        Ok(EncryptedValue { garbage: encoded })
    }

    fn decrypt_private(
        key: &KeyPair,
        encrypted: &EncryptedValue,
    ) -> Result<DecryptedValue, DecryptionError> {
        let openssl_key = key.get_openssl_key()?;
        let data_in = decode(&encrypted.garbage)?;
        let mut data_out = vec![0; openssl_key.size() as usize];
        let decrypted_len = openssl_key.private_decrypt(&data_in, &mut data_out, Padding::PKCS1)?;
        let secret = String::from_utf8(data_out[0..decrypted_len].to_vec())?;

        Ok(DecryptedValue { secret })
    }
}

#[cfg(test)]
mod unit_test {
    use crate::crypto::{CryptoService, DecryptedValue, RsaCryptoService};

    #[test]
    fn test_key_generation() {
        let key = RsaCryptoService::generate_key().unwrap();
        assert!(RsaCryptoService::verify_key(&key).unwrap());
    }

    #[test]
    fn test_private_key_encrypt_decrypt_inverse_property() {
        let key = RsaCryptoService::generate_key().unwrap();
        let input = DecryptedValue { secret: "Parth's secrets".to_string() };

        let encrypted = RsaCryptoService::encrypt_private(&key, &input).unwrap();
        let decrypted = RsaCryptoService::decrypt_public(&key, &encrypted).unwrap();

        assert_eq!(input, decrypted);
    }

    #[test]
    fn test_public_key_encrypt_decrypt_inverse_property() {
        let key = RsaCryptoService::generate_key().unwrap();
        let input = DecryptedValue { secret: "Parth's secrets".to_string() };

        let encrypted = RsaCryptoService::encrypt_public(&key, &input).unwrap();
        let decrypted = RsaCryptoService::decrypt_private(&key, &encrypted).unwrap();

        assert_eq!(input, decrypted);
    }

    #[test]
    fn test_private_key_encrypt_decrypt_inverse_property_small_input() {
        let key = RsaCryptoService::generate_key().unwrap();
        let input = DecryptedValue { secret: "".to_string() };

        let encrypted = RsaCryptoService::encrypt_private(&key, &input).unwrap();
        let decrypted = RsaCryptoService::decrypt_public(&key, &encrypted).unwrap();

        assert_eq!(input, decrypted);
    }

    #[test]
    fn test_public_key_encrypt_decrypt_inverse_property_small_input() {
        let key = RsaCryptoService::generate_key().unwrap();
        let input = DecryptedValue { secret: "".to_string() };

        let encrypted = RsaCryptoService::encrypt_public(&key, &input).unwrap();
        let decrypted = RsaCryptoService::decrypt_private(&key, &encrypted).unwrap();

        assert_eq!(input, decrypted);
    }

    static LARGE_TEXT: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Aenean et tortor at risus viverra adipiscing at in. Commodo ullamcorper a lacus vestibulum sed arcu. Etiam erat velit scelerisque in dictum non. Ullamcorper morbi tincidunt ornare massa eget. Leo vel fringilla est ullamcorper eget nulla. Donec ultrices tincidunt arcu non sodales. Non odio euismod lacinia at. Sollicitudin aliquam ultrices sagittis orci a. Tincidunt praesent semper feugiat nibh sed. Magna fermentum iaculis eu non. Faucibus purus in massa tempor nec feugiat. Ac feugiat sed lectus vestibulum. Volutpat lacus laoreet non curabitur.";

    #[test]
    fn test_private_key_encrypt_decrypt_inverse_property_large_input() {
        let key = RsaCryptoService::generate_key().unwrap();
        let input = DecryptedValue { secret: LARGE_TEXT.to_string() };

        assert!(RsaCryptoService::encrypt_private(&key, &input).is_err());
    }

    #[test]
    fn test_public_key_encrypt_decrypt_inverse_property_large_input() {
        let key = RsaCryptoService::generate_key().unwrap();
        let input = DecryptedValue { secret: LARGE_TEXT.to_string() };

        assert!(RsaCryptoService::encrypt_public(&key, &input).is_err());
    }
}
