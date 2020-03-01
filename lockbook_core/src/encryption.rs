extern crate openssl;

use openssl::rsa::Rsa;

use crate::encryption::Error::KeyGenerationError;

use self::openssl::pkey::Private;

pub trait Encryption {
    fn generate_key() -> Result<Rsa<Private>, Error>;
}

pub struct EncryptionImpl;

pub enum Error {
    KeyGenerationError
}

impl Encryption for EncryptionImpl {
    fn generate_key() -> Result<Rsa<Private>, Error> {
        match Rsa::generate(2048) {
            Ok(rsa) => Ok(rsa),
            Err(_) => Err(KeyGenerationError),
        }
    }
}