use rsa::RSAPublicKey;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeFileContentRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub old_metadata_version: u64,
    pub new_file_content: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeFileContentResponse {
    pub current_metadata_and_content_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateFileResponse {
    pub current_metadata_and_content_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum CreateFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileIdTaken,
    FilePathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub old_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFileResponse {
    pub current_metadata_and_content_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum DeleteFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    EditConflict,
    FileDeleted,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetPublicKeyRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetPublicKeyResponse {
    pub key: RSAPublicKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetPublicKeyError {
    InternalError,
    UserNotFound,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUpdatesRequest {
    pub username: String,
    pub auth: String,
    pub since_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUpdatesResponse {
    pub file_metadata: Vec<FileMetadata>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetUpdatesError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FileMetadata {
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content_version: u64,
    pub file_metadata_version: u64,
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub old_metadata_version: u64,
    pub new_file_path: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveFileResponse {
    pub current_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum MoveFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    EditConflict,
    FileDeleted,
    FilePathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct NewAccountRequest {
    pub username: String,
    pub auth: String,
    pub public_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct NewAccountResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum NewAccountError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    UsernameTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub old_metadata_version: u64,
    pub new_file_name: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameFileResponse {
    pub current_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum RenameFileError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    FileDeleted,
    EditConflict,
}
