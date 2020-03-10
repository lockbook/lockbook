extern crate openssl;

use std::ops::Try;
use std::option::NoneError;
use base64::encode;

use openssl::rsa::Rsa;

use crate::error_enum;

use self::openssl::error::ErrorStack;

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

        let n = encode(&their_key.n().to_vec());
        let e = encode(&their_key.e().to_vec());
        let d = encode(&their_key.d().to_vec());
        let p = encode(&their_key.p().into_result()?.to_vec());
        let q = encode(&their_key.q().into_result()?.to_vec());
        let dmp1 = encode(&their_key.dmp1().into_result()?.to_vec());
        let dmq1 = encode(&their_key.dmq1().into_result()?.to_vec());
        let iqmp = encode(&their_key.iqmp().into_result()?.to_vec());

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
