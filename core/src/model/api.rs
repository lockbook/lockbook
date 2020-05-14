use serde::{Deserialize, Serialize};
use rsa::RSAPublicKey;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ChangeFileContentRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub old_file_version: u64,
    pub new_file_content: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ChangeFileContentResponse {
    pub current_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum ChangeFileContentError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    EditConflict,
    FileDeleted,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CreateFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CreateFileResponse {
    pub current_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum CreateFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileIdTaken,
    FilePathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DeleteFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DeleteFileResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum DeleteFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    FileDeleted,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct GetPublicKeyRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct GetPublicKeyResponse {
    pub key: RSAPublicKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum GetPublicKeyError {
    InternalError,
    UserNotFound,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct GetUpdatesRequest {
    pub username: String,
    pub auth: String,
    pub since_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct GetUpdatesResponse {
    pub file_metadata: Vec<FileMetadata>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum GetUpdatesError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct FileMetadata {
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content_version: u64,
    pub file_metadata_version: u64,
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MoveFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub new_file_path: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MoveFileResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum MoveFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    FileDeleted,
    FilePathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct NewAccountRequest {
    pub username: String,
    pub auth: String,
    pub public_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct NewAccountResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum NewAccountError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    UsernameTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RenameFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub new_file_name: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RenameFileResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum RenameFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    FileDeleted,
}
