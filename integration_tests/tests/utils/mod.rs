use std::env;

use uuid::Uuid;

use lockbook_core::client::change_file_content;
use lockbook_core::client::create_file;
use lockbook_core::client::delete_file;
use lockbook_core::client::get_public_key;
use lockbook_core::client::get_updates;
use lockbook_core::client::move_file;
use lockbook_core::client::new_account;
use lockbook_core::client::rename_file;
use lockbook_core::model::account::Account;
use lockbook_core::service::crypto_service::{PubKeyCryptoService, RsaImpl};

pub fn api_loc() -> String {
    format!(
        "http://{}:{}",
        env_or_panic("SERVER_HOST"),
        env_or_panic("SERVER_PORT")
    )
}

fn env_or_panic(var_name: &str) -> String {
    env::var(var_name).expect(&format!("Missing environment variable {}", var_name))
}

pub fn generate_account() -> Account {
    Account {
        username: generate_username(),
        keys: RsaImpl::generate_key().unwrap(),
    }
}

pub fn generate_username() -> String {
    Uuid::new_v4().to_string().trim_matches("-").to_string()
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
    NewAccountError(new_account::Error),
    CreateFileError(create_file::Error),
    ChangeFileContentError(change_file_content::Error),
    RenameFileError(rename_file::Error),
    MoveFileError(move_file::Error),
    DeleteFileError(delete_file::Error),
    GetUpdatesError(get_updates::Error),
    GetPublicKeyError(get_public_key::Error),
}

impl From<new_account::Error> for TestError {
    fn from(e: new_account::Error) -> TestError {
        TestError::NewAccountError(e)
    }
}

impl From<create_file::Error> for TestError {
    fn from(e: create_file::Error) -> TestError {
        TestError::CreateFileError(e)
    }
}

impl From<change_file_content::Error> for TestError {
    fn from(e: change_file_content::Error) -> TestError {
        TestError::ChangeFileContentError(e)
    }
}

impl From<rename_file::Error> for TestError {
    fn from(e: rename_file::Error) -> TestError {
        TestError::RenameFileError(e)
    }
}

impl From<move_file::Error> for TestError {
    fn from(e: move_file::Error) -> TestError {
        TestError::MoveFileError(e)
    }
}

impl From<delete_file::Error> for TestError {
    fn from(e: delete_file::Error) -> TestError {
        TestError::DeleteFileError(e)
    }
}

impl From<get_updates::Error> for TestError {
    fn from(e: get_updates::Error) -> TestError {
        TestError::GetUpdatesError(e)
    }
}

impl From<get_public_key::Error> for TestError {
    fn from(e: get_public_key::Error) -> TestError {
        TestError::GetPublicKeyError(e)
    }
}
