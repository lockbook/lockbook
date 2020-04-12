extern crate lockbook_core;
use lockbook_core::lockbook_api::ChangeFileContentError;
use lockbook_core::lockbook_api::CreateFileError;
use lockbook_core::lockbook_api::DeleteFileError;
use lockbook_core::lockbook_api::GetUpdatesError;
use lockbook_core::lockbook_api::MoveFileError;
use lockbook_core::lockbook_api::NewAccountError;
use lockbook_core::lockbook_api::RenameFileError;
use std::env;
use uuid::Uuid;

pub fn api_loc() -> String {
    match env::var("LOCKBOOK_API_LOCATION") {
        Ok(s) => s,
        Err(e) => panic!(
            "Could not read environment variable LOCKBOOK_API_LOCATION: {}",
            e
        ),
    }
}

pub fn generate_username() -> String {
    Uuid::new_v4().to_string()
}

pub fn generate_file_id() -> String {
    Uuid::new_v4().to_string()
}

macro_rules! assert_matches(
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