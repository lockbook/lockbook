pub mod change_file_content;
pub mod create_file;
pub mod delete_file;
pub mod get_file;
pub mod get_updates;
pub mod move_file;
pub mod new_account;
pub mod rename_file;
pub mod get_public_key;

use crate::model::api::{
    ChangeFileContentRequest, CreateFileRequest, DeleteFileRequest, FileMetadata, GetPublicKeyRequest,
    GetUpdatesRequest, MoveFileRequest, NewAccountRequest, RenameFileRequest,
};
use crate::service::file_encryption_service::EncryptedFile;
use crate::{API_LOC, BUCKET_LOC};
use rsa::RSAPublicKey;

pub trait Client {
    fn change_file_content(
        username: String,
        auth: String,
        file_id: String,
        old_file_version: u64,
        new_file_content: String,
    ) -> Result<u64, change_file_content::Error>;
    fn create_file(
        username: String,
        auth: String,
        file_id: String,
        file_name: String,
        file_path: String,
        file_content: String,
    ) -> Result<u64, create_file::Error>;
    fn delete_file(
        username: String,
        auth: String,
        file_id: String,
    ) -> Result<(), delete_file::Error>;
    fn get_updates(
        username: String,
        auth: String,
        since_version: u64,
    ) -> Result<Vec<FileMetadata>, get_updates::Error>;
    fn move_file(
        username: String,
        auth: String,
        file_id: String,
        new_file_path: String,
    ) -> Result<(), move_file::Error>;
    fn new_account(
        username: String,
        auth: String,
        public_key: String,
    ) -> Result<(), new_account::Error>;
    fn rename_file(
        username: String,
        auth: String,
        file_id: String,
        new_file_name: String,
    ) -> Result<(), rename_file::Error>;
    fn get_file(file_id: String) -> Result<EncryptedFile, get_file::Error>;
    fn get_public_key(username: String) -> Result<RSAPublicKey, get_public_key::Error>;
}

pub struct ClientImpl;
impl Client for ClientImpl {
    fn change_file_content(
        username: String,
        auth: String,
        file_id: String,
        old_file_version: u64,
        new_file_content: String,
    ) -> Result<u64, change_file_content::Error> {
        Ok(change_file_content::send(
            String::from(API_LOC),
            &ChangeFileContentRequest {
                username: username,
                auth: auth,
                file_id: file_id,
                old_file_version: old_file_version,
                new_file_content: new_file_content,
            },
        )?
        .current_version)
    }
    fn create_file(
        username: String,
        auth: String,
        file_id: String,
        file_name: String,
        file_path: String,
        file_content: String,
    ) -> Result<u64, create_file::Error> {
        Ok(create_file::send(
            String::from(API_LOC),
            &CreateFileRequest {
                username: username,
                auth: auth,
                file_id: file_id,
                file_name: file_name,
                file_path: file_path,
                file_content: file_content,
            },
        )?
        .current_version)
    }
    fn delete_file(
        username: String,
        auth: String,
        file_id: String,
    ) -> Result<(), delete_file::Error> {
        delete_file::send(
            String::from(API_LOC),
            &DeleteFileRequest {
                username: username,
                auth: auth,
                file_id: file_id,
            },
        )?;
        Ok(())
    }
    fn get_public_key(
        username: String,
    ) -> Result<RSAPublicKey, get_public_key::Error> {
        Ok(get_public_key::send(
            String::from(API_LOC),
            &GetPublicKeyRequest {
                username: username,
            },
        )?
        .key)
    }
    fn get_updates(
        username: String,
        auth: String,
        since_version: u64,
    ) -> Result<Vec<FileMetadata>, get_updates::Error> {
        Ok(get_updates::send(
            String::from(API_LOC),
            &GetUpdatesRequest {
                username: username,
                auth: auth,
                since_version: since_version,
            },
        )?
        .file_metadata)
    }
    fn move_file(
        username: String,
        auth: String,
        file_id: String,
        new_file_path: String,
    ) -> Result<(), move_file::Error> {
        move_file::send(
            String::from(API_LOC),
            &MoveFileRequest {
                username: username,
                auth: auth,
                file_id: file_id,
                new_file_path: new_file_path,
            },
        )?;
        Ok(())
    }
    fn new_account(
        username: String,
        auth: String,
        public_key: String,
    ) -> Result<(), new_account::Error> {
        new_account::send(
            String::from(API_LOC),
            &NewAccountRequest {
                username: username,
                auth: auth,
                public_key: public_key,
            },
        )?;
        Ok(())
    }
    fn rename_file(
        username: String,
        auth: String,
        file_id: String,
        new_file_name: String,
    ) -> Result<(), rename_file::Error> {
        rename_file::send(
            String::from(API_LOC),
            &RenameFileRequest {
                username: username,
                auth: auth,
                file_id: file_id,
                new_file_name: new_file_name,
            },
        )?;
        Ok(())
    }
    fn get_file(file_id: String) -> Result<EncryptedFile, get_file::Error> {
        get_file::send(String::from(BUCKET_LOC), file_id)
    }
}
