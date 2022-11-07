pub mod access_info;
pub mod account;
pub mod api;
pub mod clock;
pub mod compression_service;
pub mod core_config;
pub mod core_ops;
pub mod crypto;
pub mod document_repo;
pub mod drawing;
pub mod file;
pub mod path_ops;
pub mod pubkey;
pub mod server_ops;
pub mod symkey;
pub mod tree;
pub mod usage;
pub mod validate;
pub mod work_unit;

use hmac::crypto_mac::{InvalidKeyLength, MacError};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;
use uuid::Uuid;

pub type SharedResult<T> = Result<T, SharedError>;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Eq)]
pub enum ValidationFailure {
    Orphan(Uuid),
    Cycle(HashSet<Uuid>),
    PathConflict(HashSet<Uuid>),
    NonFolderWithChildren(Uuid),
    FileWithDifferentOwnerParent(Uuid),
    NonDecryptableFileName(Uuid),
    SharedLink { link: Uuid, shared_ancestor: Uuid },
    DuplicateLink { target: Uuid },
    BrokenLink(Uuid),
    OwnedLink(Uuid),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SharedError {
    InsufficientPermission, // todo: this is a duplicate of NotPermissioned
    PathContainsEmptyFileName,
    PathTaken,
    RootNonexistent,
    FileNonexistent,
    FileNameContainsSlash,
    RootModificationInvalid,
    FileNameEmpty,
    FileParentNonexistent,
    FileNotFolder,
    FileNotDocument,
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
    ShareNonexistent,
    SharedSecretUnexpectedSize,
    SharedSecretError(libsecp256k1::Error),
    ValidationFailure(ValidationFailure),

    /// Arises during a call to upsert, when the caller does not have the correct old version of the
    /// File they're trying to modify
    OldVersionIncorrect,

    /// Arises during a call to upsert, when the old file is not known to the server
    OldFileNotFound,

    /// Arises during a call to upsert, when the caller suggests that a file is new, but the id already
    /// exists
    OldVersionRequired,

    /// Arises during a call to upsert, when the person making the request is not an owner of the file
    /// or has not signed the update
    NotPermissioned,

    /// Arises during a call to upsert, when a diff's new.id != old.id
    DiffMalformed,

    /// Metas in upsert cannot contain changes to digest
    HmacModificationInvalid,

    /// Found update to a deleted file
    DeletedFileUpdated,

    Io(String),

    Unexpected(&'static str),
}

impl From<bincode::Error> for SharedError {
    fn from(err: bincode::Error) -> Self {
        Self::BincodeError(err.to_string())
    }
}

impl From<io::Error> for SharedError {
    fn from(err: io::Error) -> Self {
        Self::Io(err.to_string())
    }
}
