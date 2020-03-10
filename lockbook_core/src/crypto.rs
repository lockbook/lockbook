extern crate openssl;

use std::ops::Try;
use std::option::NoneError;

use openssl::rsa::Rsa;

use crate::error_enum;

use self::openssl::error::ErrorStack;

#[derive(PartialEq, Debug)]
pub struct PublicKey {
    pub n: Vec<u8>,
    pub e: Vec<u8>,
}

#[derive(PartialEq, Debug)]
pub struct PrivateKey {
    pub d: Vec<u8>,
    pub p: Vec<u8>,
    pub q: Vec<u8>,
    pub dmp1: Vec<u8>,
    pub dmq1: Vec<u8>,
    pub iqmp: Vec<u8>,
}

#[derive(PartialEq, Debug)]
pub struct KeyPair {
    pub public_key: PublicKey,
    pub private_key: PrivateKey,
}

error_enum! {
    enum Error {
        KeyGenerationError(ErrorStack),
        KeyComponentMissing(NoneError),

    }
}

pub trait CryptoService {
    fn generate_key() -> Result<KeyPair, Error>;
}

pub struct RsaCryptoService;

impl CryptoService for RsaCryptoService {
    fn generate_key() -> Result<KeyPair, Error> {
        let their_key = Rsa::generate(2048)?;

        let n = their_key.n().to_vec();
        let e = their_key.e().to_vec();
        let d = their_key.d().to_vec();
        let p = their_key.p().into_result()?.to_vec();
        let q = their_key.q().into_result()?.to_vec();
        let dmp1 = their_key.dmp1().into_result()?.to_vec();
        let dmq1 = their_key.dmq1().into_result()?.to_vec();
        let iqmp = their_key.iqmp().into_result()?.to_vec();

        Ok(
            KeyPair {
                public_key: PublicKey { n, e },
                private_key: PrivateKey {
                    d,
                    p,
                    q,
                    dmp1,
                    dmq1,
                    iqmp,
                },
            }
        )
    }
}

#[cfg(test)]
mod unit_test {
    use openssl::rsa::Rsa;

    #[test]
    fn test_key_generation() {
        let their_key = Rsa::generate(2048).unwrap();
        println!("{:?}", their_key.public_key_to_pem())
    }
}
