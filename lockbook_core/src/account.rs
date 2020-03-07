use std::ops::Try;
use std::option::NoneError;

use openssl::pkey::Private;
use openssl::rsa::Rsa;

use crate::error_enum;

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
pub struct Account {
    pub username: String,
    pub public_key: PublicKey,
    pub private_key: PrivateKey,
}

error_enum! {
    enum Error {
        KeyComponentMissing(NoneError)
    }
}

impl Account {
    pub fn new(username: String, keypair: Rsa<Private>) -> Result<Account, Error> {
        let n = keypair.n().to_vec();
        let e = keypair.e().to_vec();
        let d = keypair.d().to_vec();
        let p = keypair.p().into_result()?.to_vec();
        let q = keypair.q().into_result()?.to_vec();
        let dmp1 = keypair.dmp1().into_result()?.to_vec();
        let dmq1 = keypair.dmq1().into_result()?.to_vec();
        let iqmp = keypair.iqmp().into_result()?.to_vec();

        Ok(
            Account {
                username,
                public_key: PublicKey { n, e },
                private_key: PrivateKey { d, p, q, dmp1, dmq1, iqmp },
            }
        )
    }
}
