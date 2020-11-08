use crate::model::account::{Account, Username};
use crate::model::crypto::*;
use crate::model::file_metadata::FileMetadata;
use reqwest::Method;
use rsa::RSAPublicKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub trait Request: Serialize {
    type Response: DeserializeOwned;
    type Error: DeserializeOwned;
    fn method() -> Method;
    fn endpoint() -> &'static str;
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocumentContentRequest {
    pub username: String,
    pub id: Uuid,
    pub old_metadata_version: u64,
    pub new_content: EncryptedDocument,
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
    ClientUpdateRequired,
}

impl Request for ChangeDocumentContentRequest {
    type Response = ChangeDocumentContentResponse;
    type Error = ChangeDocumentContentError;
    fn method() -> Method {
        Method::PUT
    }
    fn endpoint() -> &'static str {
        "change-document-content"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateDocumentRequest {
    pub username: String,
    pub id: Uuid,
    pub name: String,
    pub parent: Uuid,
    pub content: EncryptedDocument,
    pub parent_access_key: FolderAccessInfo,
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
    ParentNotFound,
    ClientUpdateRequired,
}

impl CreateDocumentRequest {
    pub fn new(username: &str, file_metadata: &FileMetadata, content: EncryptedDocument) -> Self {
        CreateDocumentRequest {
            username: String::from(username),
            id: file_metadata.id,
            name: file_metadata.name.clone(),
            parent: file_metadata.parent,
            content,
            parent_access_key: file_metadata.folder_access_keys.clone(),
        }
    }
}

impl Request for CreateDocumentRequest {
    type Response = CreateDocumentResponse;
    type Error = CreateDocumentError;
    fn method() -> Method {
        Method::POST
    }
    fn endpoint() -> &'static str {
        "create-document"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteDocumentRequest {
    pub username: String,
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
    ClientUpdateRequired,
}

impl Request for DeleteDocumentRequest {
    type Response = DeleteDocumentResponse;
    type Error = DeleteDocumentError;
    fn method() -> Method {
        Method::DELETE
    }
    fn endpoint() -> &'static str {
        "delete-document"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveDocumentRequest {
    pub username: String,
    pub id: Uuid,
    pub old_metadata_version: u64,
    pub new_parent: Uuid,
    pub new_folder_access: FolderAccessInfo,
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
    ParentNotFound,
    EditConflict,
    DocumentDeleted,
    DocumentPathTaken,
    ClientUpdateRequired,
}

impl MoveDocumentRequest {
    pub fn new(username: &str, file_metadata: &FileMetadata) -> Self {
        MoveDocumentRequest {
            username: String::from(username),
            id: file_metadata.id,
            old_metadata_version: file_metadata.metadata_version,
            new_parent: file_metadata.parent,
            new_folder_access: file_metadata.folder_access_keys.clone(),
        }
    }
}

impl Request for MoveDocumentRequest {
    type Response = MoveDocumentResponse;
    type Error = MoveDocumentError;
    fn method() -> Method {
        Method::PUT
    }
    fn endpoint() -> &'static str {
        "move-document"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameDocumentRequest {
    pub username: String,
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
    DocumentPathTaken,
    ClientUpdateRequired,
}

impl RenameDocumentRequest {
    pub fn new(username: &str, file_metadata: &FileMetadata) -> Self {
        RenameDocumentRequest {
            username: String::from(username),
            id: file_metadata.id,
            old_metadata_version: file_metadata.metadata_version,
            new_name: file_metadata.name.clone(),
        }
    }
}

impl Request for RenameDocumentRequest {
    type Response = RenameDocumentResponse;
    type Error = RenameDocumentError;
    fn method() -> Method {
        Method::PUT
    }
    fn endpoint() -> &'static str {
        "rename-document"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetDocumentRequest {
    pub id: Uuid,
    pub content_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetDocumentResponse {
    pub content: EncryptedDocument,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetDocumentError {
    InternalError,
    DocumentNotFound,
    ClientUpdateRequired,
}

impl Request for GetDocumentRequest {
    type Response = GetDocumentResponse;
    type Error = GetDocumentError;
    fn method() -> Method {
        Method::GET
    }
    fn endpoint() -> &'static str {
        "get-document"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateFolderRequest {
    pub username: String,
    pub id: Uuid,
    pub name: String,
    pub parent: Uuid,
    pub parent_access_key: FolderAccessInfo,
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
    ParentNotFound,
    ClientUpdateRequired,
}

impl CreateFolderRequest {
    pub fn new(username: &str, file_metadata: &FileMetadata) -> Self {
        CreateFolderRequest {
            username: String::from(username),
            id: file_metadata.id,
            name: file_metadata.name.clone(),
            parent: file_metadata.parent,
            parent_access_key: file_metadata.folder_access_keys.clone(),
        }
    }
}

impl Request for CreateFolderRequest {
    type Response = CreateFolderResponse;
    type Error = CreateFolderError;
    fn method() -> Method {
        Method::POST
    }
    fn endpoint() -> &'static str {
        "create-folder"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFolderRequest {
    pub username: String,
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
    ClientUpdateRequired,
}

impl Request for DeleteFolderRequest {
    type Response = DeleteFolderResponse;
    type Error = DeleteFolderError;
    fn method() -> Method {
        Method::DELETE
    }
    fn endpoint() -> &'static str {
        "delete-folder"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveFolderRequest {
    pub username: String,
    pub id: Uuid,
    pub old_metadata_version: u64,
    pub new_parent: Uuid,
    pub new_folder_access: FolderAccessInfo,
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
    ParentNotFound,
    EditConflict,
    FolderDeleted,
    FolderPathTaken,
    ClientUpdateRequired,
}

impl MoveFolderRequest {
    pub fn new(username: &str, file_metadata: &FileMetadata) -> Self {
        MoveFolderRequest {
            username: String::from(username),
            id: file_metadata.id,
            old_metadata_version: file_metadata.metadata_version,
            new_parent: file_metadata.parent,
            new_folder_access: file_metadata.folder_access_keys.clone(),
        }
    }
}

impl Request for MoveFolderRequest {
    type Response = MoveFolderResponse;
    type Error = MoveFolderError;
    fn method() -> Method {
        Method::PUT
    }
    fn endpoint() -> &'static str {
        "move-folder"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameFolderRequest {
    pub username: String,
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
    FolderPathTaken,
    ClientUpdateRequired,
}

impl RenameFolderRequest {
    pub fn new(username: &str, file_metadata: &FileMetadata) -> Self {
        RenameFolderRequest {
            username: String::from(username),
            id: file_metadata.id,
            old_metadata_version: file_metadata.metadata_version,
            new_name: file_metadata.name.clone(),
        }
    }
}

impl Request for RenameFolderRequest {
    type Response = RenameFolderResponse;
    type Error = RenameFolderError;
    fn method() -> Method {
        Method::PUT
    }
    fn endpoint() -> &'static str {
        "rename-folder"
    }
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
    ClientUpdateRequired,
}

impl Request for GetPublicKeyRequest {
    type Response = GetPublicKeyResponse;
    type Error = GetPublicKeyError;
    fn method() -> Method {
        Method::GET
    }
    fn endpoint() -> &'static str {
        "get-public-key"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUsageRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUsageResponse {
    pub usages: Vec<FileUsage>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FileUsage {
    pub file_id: String,
    pub byte_secs: u64,
    pub secs: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetUsageError {
    InternalError,
    InvalidUsername,
    UserNotFound,
    ClientUpdateRequired,
}

impl Request for GetUsageRequest {
    type Response = GetUsageResponse;
    type Error = GetUsageError;
    fn method() -> Method {
        Method::GET
    }
    fn endpoint() -> &'static str {
        "get-usage"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUpdatesRequest {
    pub username: String,
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
    ClientUpdateRequired,
}

impl Request for GetUpdatesRequest {
    type Response = GetUpdatesResponse;
    type Error = GetUpdatesError;
    fn method() -> Method {
        Method::GET
    }
    fn endpoint() -> &'static str {
        "get-updates"
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct NewAccountRequest {
    pub username: Username,
    pub public_key: RSAPublicKey,
    pub folder_id: Uuid,
    pub parent_access_key: FolderAccessInfo,
    pub user_access_key: EncryptedUserAccessKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct NewAccountResponse {
    pub folder_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum NewAccountError {
    InternalError,
    InvalidAuth,
    ExpiredAuth,
    UsernameTaken,
    InvalidPublicKey,
    InvalidUserAccessKey,
    InvalidUsername,
    FileIdTaken,
    ClientUpdateRequired,
}

impl NewAccountRequest {
    pub fn new(account: &Account, file_metadata: &FileMetadata) -> Self {
        NewAccountRequest {
            username: account.username.clone(),
            public_key: account.private_key.to_public_key(),
            folder_id: file_metadata.id,
            parent_access_key: file_metadata.folder_access_keys.clone(),
            user_access_key: file_metadata
                .user_access_keys
                .get(&account.username)
                .unwrap() // TODO: compiler guarantee for this
                .access_key
                .clone(),
        }
    }
}

impl Request for NewAccountRequest {
    type Response = NewAccountResponse;
    type Error = NewAccountError;
    fn method() -> Method {
        Method::POST
    }
    fn endpoint() -> &'static str {
        "new-account"
    }
}
