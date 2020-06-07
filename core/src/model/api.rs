use rsa::RSAPublicKey;
use serde::{Deserialize, Serialize};

use crate::model::aliases::*;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeFileContentRequest {
    pub username: Username,
    pub auth: Signature,
    pub file_id: FileId,
    pub old_metadata_version: Version,
    pub new_file_content: EncryptedDocumentContent,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeFileContentResponse {
    pub current_metadata_and_content_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ChangeFileContentError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    EditConflict,
    FileDeleted,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateFileRequest {
    pub username: Username,
    pub auth: Signature,
    pub file_id: FileId,
    pub file_name: FileName,
    pub file_parent: FileId,
    pub file_content: EncryptedDocumentContent,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateFileResponse {
    pub current_metadata_and_content_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum CreateFileError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileIdTaken,
    FilePathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFileRequest {
    pub username: Username,
    pub auth: Signature,
    pub file_id: FileId,
    pub old_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFileResponse {
    pub current_metadata_and_content_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum DeleteFileError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    EditConflict,
    FileDeleted,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetPublicKeyRequest {
    pub username: Username,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetPublicKeyResponse {
    pub key: RSAPublicKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetPublicKeyError {
    InternalError,
    InvalidUsername,
    UserNotFound,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUpdatesRequest {
    pub username: Username,
    pub auth: Signature,
    pub since_metadata_version: Version,
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
    InvalidUsername,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FileMetadata {
    pub file_id: FileId,
    pub file_name: FileName,
    pub file_parent: FileId,
    pub file_content_version: Version,
    pub file_metadata_version: Version,
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveFileRequest {
    pub username: Username,
    pub auth: Signature,
    pub file_id: FileId,
    pub old_metadata_version: Version,
    pub new_file_parent: FileId,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveFileResponse {
    pub current_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum MoveFileError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
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
    pub username: Username,
    pub auth: Signature,
    pub public_key: RSAPublicKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct NewAccountResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum NewAccountError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    UsernameTaken,
    InvalidPublicKey,
    InvalidUsername,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameFileRequest {
    pub username: Username,
    pub auth: Signature,
    pub file_id: FileId,
    pub old_metadata_version: Version,
    pub new_file_name: FileName,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameFileResponse {
    pub current_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum RenameFileError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileNotFound,
    FileDeleted,
    EditConflict,
}
