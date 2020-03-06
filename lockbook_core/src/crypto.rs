extern crate openssl;

use openssl::rsa::Rsa;

use crate::crypto::Error::KeyGenerationError;
use crate::error_enum;

use self::openssl::error::ErrorStack;
use self::openssl::pkey::Private;

error_enum! {
    enum Error {
        KeyGenerationError(ErrorStack),
    }
}

pub trait CryptoService {
    fn generate_key() -> Result<Rsa<Private>, Error>;
}

pub struct RsaCryptoService;


impl CryptoService for RsaCryptoService {
    fn generate_key() -> Result<Rsa<Private>, Error> {
        Ok(Rsa::generate(2048)?)
    }
}
