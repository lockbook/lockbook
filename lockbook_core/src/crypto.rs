extern crate openssl;

use openssl::rsa::Rsa;

use crate::crypto::Error::KeyGenerationError;

use self::openssl::pkey::Private;
use self::openssl::error::ErrorStack;

pub trait CryptoService {
    fn generate_key() -> Result<Rsa<Private>, Error>;
}

pub struct RsaCryptoService;

pub enum Error {
    KeyGenerationError(ErrorStack)
}

impl CryptoService for RsaCryptoService {
    fn generate_key() -> Result<Rsa<Private>, Error> {
        match Rsa::generate(2048) {
            Ok(rsa) => Ok(rsa),
            Err(e) => Err(KeyGenerationError(e)),
        }
    }
}