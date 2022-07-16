use hmac::crypto_mac::{InvalidKeyLength, MacError};

pub mod access_info;
pub mod account;
pub mod api;
pub mod clock;
pub mod crypto;
pub mod drawing;
pub mod file_like;
pub mod file_metadata;
pub mod pubkey;
pub mod secret_filename;
pub mod server_file;
pub mod signed_file;
pub mod symkey;
pub mod tree_like;
pub mod utils;
pub mod work_unit;

type SharedResult<T> = Result<T, SharedError>;

#[derive(Debug)]
pub enum SharedError {
    SignatureInvalid,
    WrongPublicKey,
    SignatureInTheFuture(u64),
    SignatureExpired(u64),
    Serialization(bincode::Error),
    Deserialization(bincode::Error),
    Encryption(aead::Error),
    HmacCreationError(InvalidKeyLength),
    Decryption(aead::Error),
    HmacValidationError(MacError),
    ParseError(libsecp256k1::Error),
    SharedSecretUnexpectedSize,
    SharedSecretError(libsecp256k1::Error),
}
