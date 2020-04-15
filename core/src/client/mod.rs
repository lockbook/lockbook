mod change_file_content;
mod create_file;
mod delete_file;
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
pub use self::get_updates::{get_updates, FileMetadata, GetUpdatesError, GetUpdatesRequest};
pub use self::move_file::{move_file, MoveFileError, MoveFileRequest, MoveFileResponse};
pub use self::new_account::{new_account, NewAccountError, NewAccountRequest, NewAccountResponse};
pub use self::rename_file::{rename_file, RenameFileError, RenameFileRequest, RenameFileResponse};

#[derive(Debug)]
pub enum ClientError {
    AccountError(NewAccountError),
    UpdatesError(GetUpdatesError),
}

pub trait Client {
    fn new_account(api_location: String, params: &NewAccountRequest) -> Result<(), ClientError>;

    fn get_updates(
        api_location: String,
        params: &GetUpdatesRequest,
    ) -> Result<Vec<FileMetadata>, ClientError>;
}

pub struct ClientImpl;

impl Client for ClientImpl {
    fn new_account(api_location: String, params: &NewAccountRequest) -> Result<(), ClientError> {
        new_account(api_location, params).map_err(|err| ClientError::AccountError(err))
    }

    fn get_updates(
        api_location: String,
        params: &GetUpdatesRequest,
    ) -> Result<Vec<FileMetadata>, ClientError> {
        get_updates(api_location, params).map_err(|err| ClientError::UpdatesError(err))
    }
}
