mod change_file_content;
mod create_file;
mod delete_file;
mod get_file;
mod get_updates;
mod move_file;
mod new_account;
mod rename_file;

pub use self::change_file_content::{
    change_file_content, ChangeFileContentError, ChangeFileContentRequest,
    ChangeFileContentResponse,
};
pub use self::create_file::{create_file, CreateFileError, CreateFileRequest, CreateFileResponse};
pub use self::delete_file::{delete_file, DeleteFileError, DeleteFileRequest, DeleteFileResponse};
pub use self::get_file::{get_file, GetFileError, GetFileRequest};
pub use self::get_updates::{get_updates, FileMetadata, GetUpdatesError, GetUpdatesRequest};
pub use self::move_file::{move_file, MoveFileError, MoveFileRequest, MoveFileResponse};
pub use self::new_account::{new_account, NewAccountError, NewAccountRequest, NewAccountResponse};
pub use self::rename_file::{rename_file, RenameFileError, RenameFileRequest, RenameFileResponse};
use crate::service::file_encryption_service::EncryptedFile;
use crate::{API_LOC, BUCKET_LOC};

#[derive(Debug)]
pub enum ClientError {
    CreateAccount(NewAccountError),
    GetUpdates(GetUpdatesError),
    CreateFile(CreateFileError),
    UpdateFile(ChangeFileContentError),
    GetFile(GetFileError),
}

pub trait Client {
    fn new_account(params: &NewAccountRequest) -> Result<(), ClientError>;
    fn get_updates(params: &GetUpdatesRequest) -> Result<Vec<FileMetadata>, ClientError>;
    fn get_file(params: &GetFileRequest) -> Result<EncryptedFile, ClientError>;
    fn create_file(params: &CreateFileRequest) -> Result<u64, ClientError>;
    fn change_file(params: &ChangeFileContentRequest) -> Result<u64, ClientError>;
}

pub struct ClientImpl;
impl Client for ClientImpl {
    fn new_account(params: &NewAccountRequest) -> Result<(), ClientError> {
        new_account(API_LOC.to_string(), params).map_err(|err| ClientError::CreateAccount(err))
    }

    fn get_updates(params: &GetUpdatesRequest) -> Result<Vec<FileMetadata>, ClientError> {
        get_updates(API_LOC.to_string(), params).map_err(|err| ClientError::GetUpdates(err))
    }

    fn get_file(params: &GetFileRequest) -> Result<EncryptedFile, ClientError> {
        get_file(BUCKET_LOC.to_string(), params).map_err(|err| ClientError::GetFile(err))
    }
    fn create_file(params: &CreateFileRequest) -> Result<u64, ClientError> {
        create_file(API_LOC.to_string(), params).map_err(|err| ClientError::CreateFile(err))
    }
    fn change_file(params: &ChangeFileContentRequest) -> Result<u64, ClientError> {
        change_file_content(API_LOC.to_string(), params).map_err(|err| ClientError::UpdateFile(err))
    }
}
