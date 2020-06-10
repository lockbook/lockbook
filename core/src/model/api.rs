use rsa::RSAPublicKey;
use serde::{Deserialize, Serialize};

use crate::model::aliases::*;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocumentContentRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: FileId,
    pub old_metadata_version: Version,
    pub new_content: EncryptedDocumentContent,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocumentContentResponse {
    pub new_metadata_and_content_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ChangeDocumentContentError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    DocumentNotFound,
    EditConflict,
    DocumentDeleted,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateDocumentRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: FileId,
    pub name: Filename,
    pub parent: FileId,
    pub content: EncryptedDocumentContent,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateDocumentResponse {
    pub new_metadata_and_content_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum CreateDocumentError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileIdTaken,
    DocumentPathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteDocumentRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: FileId,
    pub old_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteDocumentResponse {
    pub new_metadata_and_content_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum DeleteDocumentError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    DocumentNotFound,
    EditConflict,
    DocumentDeleted,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveDocumentRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: FileId,
    pub old_metadata_version: Version,
    pub new_parent: FileId,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveDocumentResponse {
    pub new_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum MoveDocumentError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    DocumentNotFound,
    EditConflict,
    DocumentDeleted,
    DocumentPathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameDocumentRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: FileId,
    pub old_metadata_version: Version,
    pub new_name: Filename,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameDocumentResponse {
    pub new_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum RenameDocumentError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    DocumentNotFound,
    DocumentDeleted,
    EditConflict,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateFolderRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: FileId,
    pub name: Filename,
    pub parent: FileId,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateFolderResponse {
    pub new_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum CreateFolderError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FileIdTaken,
    FolderPathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFolderRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: FileId,
    pub old_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFolderResponse {
    pub new_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum DeleteFolderError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FolderNotFound,
    EditConflict,
    FolderDeleted,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveFolderRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: FileId,
    pub old_metadata_version: Version,
    pub new_parent: FileId,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveFolderResponse {
    pub new_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum MoveFolderError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FolderNotFound,
    EditConflict,
    FolderDeleted,
    FolderPathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameFolderRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: FileId,
    pub old_metadata_version: Version,
    pub new_name: Filename,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameFolderResponse {
    pub new_metadata_version: Version,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum RenameFolderError {
    InternalError,
    InvalidAuth,
    InvalidUsername,
    ExpiredAuth,
    NotPermissioned,
    UserNotFound,
    FolderNotFound,
    FolderDeleted,
    EditConflict,
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
    pub signature: Signature,
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
    pub id: FileId,
    pub name: Filename,
    pub parent: FileId,
    pub content_version: Version,
    pub metadata_version: Version,
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct NewAccountRequest {
    pub username: Username,
    pub signature: Signature,
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
