extern crate core;

use bincode::Error;
use hmac::crypto_mac::{InvalidKeyLength, MacError};

pub mod access_info;
pub mod account;
pub mod api;
pub mod clock;
pub mod crypto;
pub mod drawing;
pub mod file;
pub mod file_like;
pub mod file_metadata;
pub mod file_ops;
pub mod filename;
pub mod lazy;
pub mod pubkey;
pub mod secret_filename;
pub mod server_file;
pub mod signed_file;
pub mod staged;
pub mod symkey;
pub mod transaction;
pub mod tree_like;
pub mod utils;
pub mod validate;
pub mod work_unit;

type SharedResult<T> = Result<T, SharedError>;

#[derive(Debug, PartialEq)]
pub enum SharedError {
    RootNonexistent,
    FileNonexistent,
    FileNameContainsSlash,
    RootModificationInvalid,
    FileNameEmpty,
    FileParentNonexistent,
    FileNotFolder,
    SignatureInvalid,
    WrongPublicKey,
    SignatureInTheFuture(u64),
    SignatureExpired(u64),
    BincodeError(String),
    Encryption(aead::Error),
    HmacCreationError(InvalidKeyLength),
    Decryption(aead::Error),
    HmacValidationError(MacError),
    ParseError(libsecp256k1::Error),
    SharedSecretUnexpectedSize,
    SharedSecretError(libsecp256k1::Error),
    Unexpected(&'static str),
}

impl From<Error> for SharedError {
    fn from(err: Error) -> Self {
        Self::BincodeError(err.to_string())
    }
}
