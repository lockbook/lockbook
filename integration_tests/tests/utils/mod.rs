use std::env;
use uuid::Uuid;
use lockbook_core::model::api::*;
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
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
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
    ChangeDocumentContentError(ChangeDocumentContentError),
    CreateDocumentError(CreateDocumentError),
    DeleteDocumentError(DeleteDocumentError),
    MoveDocumentError(MoveDocumentError),
    RenameDocumentError(RenameDocumentError),
    CreateFolderError(CreateFolderError),
    DeleteFolderError(DeleteFolderError),
    MoveFolderError(MoveFolderError),
    RenameFolderError(RenameFolderError),
    GetPublicKeyError(GetPublicKeyError),
    GetUpdatesError(GetUpdatesError),
    NewAccountError(NewAccountError),
}

impl From<ChangeDocumentContentError> for TestError {
    fn from<ChangeDocumentContentError>(e: ChangeDocumentContentError) {
        TestError::ChangeDocumentContentError(e)
    }
}

impl From<CreateDocumentError> for TestError {
    fn from<CreateDocumentError>(e: CreateDocumentError) {
        TestError::CreateDocumentError(e)
    }
}

impl From<DeleteDocumentError> for TestError {
    fn from<DeleteDocumentError>(e: DeleteDocumentError) {
        TestError::DeleteDocumentError(e)
    }
}

impl From<MoveDocumentError> for TestError {
    fn from<MoveDocumentError>(e: MoveDocumentError) {
        TestError::MoveDocumentError(e)
    }
}

impl From<RenameDocumentError> for TestError {
    fn from<RenameDocumentError>(e: RenameDocumentError) {
        TestError::RenameDocumentError(e)
    }
}

impl From<CreateFolderError> for TestError {
    fn from<CreateFolderError>(e: CreateFolderError) {
        TestError::CreateFolderError(e)
    }
}

impl From<DeleteFolderError> for TestError {
    fn from<DeleteFolderError>(e: DeleteFolderError) {
        TestError::DeleteFolderError(e)
    }
}

impl From<MoveFolderError> for TestError {
    fn from<MoveFolderError>(e: MoveFolderError) {
        TestError::MoveFolderError(e)
    }
}

impl From<RenameFolderError> for TestError {
    fn from<RenameFolderError>(e: RenameFolderError) {
        TestError::RenameFolderError(e)
    }
}

impl From<GetPublicKeyError> for TestError {
    fn from<GetPublicKeyError>(e: GetPublicKeyError) {
        TestError::GetPublicKeyError(e)
    }
}

impl From<GetUpdatesError> for TestError {
    fn from<GetUpdatesError>(e: GetUpdatesError) {
        TestError::GetUpdatesError(e)
    }
}

impl From<NewAccountError> for TestError {
    fn from<NewAccountError>(e: NewAccountError) {
        TestError::NewAccountError(e)
    }
}
