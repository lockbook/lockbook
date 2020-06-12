use crate::model::crypto::*;
use rsa::RSAPublicKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocumentContentRequest {
    pub username: String,
    pub signature: String,
    pub id: Uuid,
    pub old_metadata_version: u64,
    pub new_content: EncryptedFile,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocumentContentResponse {
    pub new_metadata_and_content_version: u64,
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
    pub username: String,
    pub signature: String,
    pub id: Uuid,
    pub name: String,
    pub parent: Uuid,
    pub content: EncryptedFile,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateDocumentResponse {
    pub new_metadata_and_content_version: u64,
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
    pub username: String,
    pub signature: String,
    pub id: Uuid,
    pub old_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteDocumentResponse {
    pub new_metadata_and_content_version: u64,
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
    pub username: String,
    pub signature: String,
    pub id: Uuid,
    pub old_metadata_version: u64,
    pub new_parent: Uuid,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveDocumentResponse {
    pub new_metadata_version: u64,
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
    pub username: String,
    pub signature: String,
    pub id: Uuid,
    pub old_metadata_version: u64,
    pub new_name: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameDocumentResponse {
    pub new_metadata_version: u64,
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
    pub username: String,
    pub signature: String,
    pub id: Uuid,
    pub name: String,
    pub parent: Uuid,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateFolderResponse {
    pub new_metadata_version: u64,
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
    pub username: String,
    pub signature: String,
    pub id: Uuid,
    pub old_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFolderResponse {
    pub new_metadata_version: u64,
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
    pub username: String,
    pub signature: String,
    pub id: Uuid,
    pub old_metadata_version: u64,
    pub new_parent: Uuid,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveFolderResponse {
    pub new_metadata_version: u64,
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
    pub username: String,
    pub signature: String,
    pub id: Uuid,
    pub old_metadata_version: u64,
    pub new_name: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameFolderResponse {
    pub new_metadata_version: u64,
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
    pub username: String,
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
    pub username: String,
    pub signature: String,
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
    InvalidUsername,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FileMetadata {
    pub id: Uuid,
    pub name: String,
    pub parent: Uuid,
    pub content_version: u64,
    pub metadata_version: u64,
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct NewAccountRequest {
    pub username: String,
    pub signature: String,
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
