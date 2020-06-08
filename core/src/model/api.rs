use rsa::RSAPublicKey;
use serde::{Deserialize, Serialize};

use crate::model::aliases::*;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocumentContentRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: DocumentId,
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
    pub id: DocumentId,
    pub name: DocumentName,
    pub parent: DocumentId,
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
    DocumentIdTaken,
    DocumentPathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteDocumentRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: DocumentId,
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
    pub id: DocumentId,
    pub old_metadata_version: Version,
    pub new_parent: DocumentId,
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
    pub id: DocumentId,
    pub old_metadata_version: Version,
    pub new_name: DocumentName,
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
    pub id: FolderId,
    pub name: FolderName,
    pub parent: FolderId,
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
    FolderIdTaken,
    FolderPathTaken,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFolderRequest {
    pub username: Username,
    pub signature: Signature,
    pub id: FolderId,
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
    pub id: FolderId,
    pub old_metadata_version: Version,
    pub new_parent: FolderId,
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
    pub id: FolderId,
    pub old_metadata_version: Version,
    pub new_name: FolderName,
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
pub enum FileMetadata {
    DocumentMetadata(DocumentMetadata),
    FolderMetadata(FolderMetadata),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DocumentMetadata {
    pub id: DocumentId,
    pub name: DocumentName,
    pub parent: DocumentId,
    pub content_version: Version,
    pub document_metadata_version: Version,
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FolderMetadata {
    pub id: FolderId,
    pub name: FolderName,
    pub parent: FolderId,
    pub folder_metadata_version: Version,
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
