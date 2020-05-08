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
pub use self::get_updates::{get_updates, GetUpdatesError, GetUpdatesRequest, ServerFileMetadata};
pub use self::move_file::{move_file, MoveFileError, MoveFileRequest, MoveFileResponse};
pub use self::new_account::{new_account, NewAccountError, NewAccountRequest, NewAccountResponse};
pub use self::rename_file::{rename_file, RenameFileError, RenameFileRequest, RenameFileResponse};
use crate::service::file_encryption_service::EncryptedFile;
use crate::{API_LOC, BUCKET_LOC};

pub trait Client {
    fn new_account(params: &NewAccountRequest) -> Result<(), NewAccountError>;
    fn get_updates(params: &GetUpdatesRequest) -> Result<Vec<ServerFileMetadata>, GetUpdatesError>;
    fn get_file(params: &GetFileRequest) -> Result<EncryptedFile, GetFileError>;
    fn create_file(params: &CreateFileRequest) -> Result<u64, CreateFileError>;
    fn change_file(params: &ChangeFileContentRequest) -> Result<u64, ChangeFileContentError>;
}

pub struct ClientImpl;
impl Client for ClientImpl {
    fn new_account(params: &NewAccountRequest) -> Result<(), NewAccountError> {
        new_account(API_LOC.to_string(), params)
    }

    fn get_updates(params: &GetUpdatesRequest) -> Result<Vec<ServerFileMetadata>, GetUpdatesError> {
        get_updates(API_LOC.to_string(), params)
    }

    fn get_file(params: &GetFileRequest) -> Result<EncryptedFile, GetFileError> {
        get_file(BUCKET_LOC.to_string(), params)
    }
    fn create_file(params: &CreateFileRequest) -> Result<u64, CreateFileError> {
        create_file(API_LOC.to_string(), params)
    }
    fn change_file(params: &ChangeFileContentRequest) -> Result<u64, ChangeFileContentError> {
        change_file_content(API_LOC.to_string(), params)
    }
}
