extern crate lockbook_core;

use std::env;

use uuid::Uuid;

use lockbook_core::client::ChangeFileContentError;
use lockbook_core::client::CreateFileError;
use lockbook_core::client::DeleteFileError;
use lockbook_core::client::GetPublicKeyError;
use lockbook_core::client::GetUpdatesError;
use lockbook_core::client::MoveFileError;
use lockbook_core::client::NewAccountError;
use lockbook_core::client::RenameFileError;
use lockbook_core::model::account::Account;
use lockbook_core::service::crypto_service::{PubKeyCryptoService, RsaImpl};

pub fn api_loc() -> String {
    match env::var("LOCKBOOK_API_LOCATION") {
        Ok(s) => s,
        Err(e) => panic!(
            "Could not read environment variable LOCKBOOK_API_LOCATION: {}",
            e
        ),
    }
}

pub fn generate_account() -> Account {
    Account {
        username: generate_username(),
        keys: RsaImpl::generate_key().unwrap(),
    }
}

pub fn generate_username() -> String {
    Uuid::new_v4().to_string()
}

pub fn generate_file_id() -> String {
    Uuid::new_v4().to_string()
}

macro_rules! assert_matches (
    ($actual:expr, $expected:pat) => {
        // Only compute actual once
        let actual_value = $actual;
        match actual_value {
            $expected => {},
            _ => panic!("assertion failed: {:?} did not match expectation", actual_value)
        }
    }
);

#[derive(Debug)]
pub enum TestError {
    NewAccountError(NewAccountError),
    CreateFileError(CreateFileError),
    ChangeFileContentError(ChangeFileContentError),
    RenameFileError(RenameFileError),
    MoveFileError(MoveFileError),
    DeleteFileError(DeleteFileError),
    GetUpdatesError(GetUpdatesError),
    GetPublicKeyError(GetPublicKeyError),
}

impl From<NewAccountError> for TestError {
    fn from(e: NewAccountError) -> TestError {
        TestError::NewAccountError(e)
    }
}

impl From<CreateFileError> for TestError {
    fn from(e: CreateFileError) -> TestError {
        TestError::CreateFileError(e)
    }
}

impl From<ChangeFileContentError> for TestError {
    fn from(e: ChangeFileContentError) -> TestError {
        TestError::ChangeFileContentError(e)
    }
}

impl From<RenameFileError> for TestError {
    fn from(e: RenameFileError) -> TestError {
        TestError::RenameFileError(e)
    }
}

impl From<MoveFileError> for TestError {
    fn from(e: MoveFileError) -> TestError {
        TestError::MoveFileError(e)
    }
}

impl From<DeleteFileError> for TestError {
    fn from(e: DeleteFileError) -> TestError {
        TestError::DeleteFileError(e)
    }
}

impl From<GetUpdatesError> for TestError {
    fn from(e: GetUpdatesError) -> TestError {
        TestError::GetUpdatesError(e)
    }
}

impl From<GetPublicKeyError> for TestError {
    fn from(e: GetPublicKeyError) -> TestError {
        TestError::GetPublicKeyError(e)
    }
}
