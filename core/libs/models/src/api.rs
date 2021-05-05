use crate::account::{Account, Username};
use crate::crypto::*;
use crate::file_metadata::FileMetadata;
use reqwest::Method;
use rsa::RSAPublicKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub trait Request {
    type Response;
    type Error;
    const METHOD: Method;
    const ROUTE: &'static str;
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RequestWrapper<T: Request> {
    pub signed_request: RSASigned<T>,
    pub client_version: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ErrorWrapper<E> {
    Endpoint(E),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocumentContentRequest {
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
    InvalidUsername,
    NotPermissioned,
    UserNotFound,
    DocumentNotFound,
    EditConflict,
    DocumentDeleted,
}

impl Request for ChangeDocumentContentRequest {
    type Response = ChangeDocumentContentResponse;
    type Error = ChangeDocumentContentError;
    const METHOD: Method = Method::PUT;
    const ROUTE: &'static str = "/change-document-content";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateDocumentRequest {
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
    InvalidUsername,
    NotPermissioned,
    UserNotFound,
    FileIdTaken,
    DocumentPathTaken,
    ParentNotFound,
}

impl CreateDocumentRequest {
    pub fn new(file_metadata: &FileMetadata, content: EncryptedDocument) -> Self {
        CreateDocumentRequest {
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
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/create-document";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteDocumentRequest {
    pub id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteDocumentResponse {
    pub new_metadata_and_content_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum DeleteDocumentError {
    InvalidUsername,
    NotPermissioned,
    UserNotFound,
    DocumentNotFound,
    EditConflict,
    DocumentDeleted,
}

impl Request for DeleteDocumentRequest {
    type Response = DeleteDocumentResponse;
    type Error = DeleteDocumentError;
    const METHOD: Method = Method::DELETE;
    const ROUTE: &'static str = "/delete-document";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveDocumentRequest {
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
    InvalidUsername,
    NotPermissioned,
    UserNotFound,
    DocumentNotFound,
    ParentNotFound,
    ParentDeleted,
    FolderMovedIntoItself,
    EditConflict,
    DocumentDeleted,
    DocumentPathTaken,
}

impl MoveDocumentRequest {
    pub fn new(file_metadata: &FileMetadata) -> Self {
        MoveDocumentRequest {
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
    const METHOD: Method = Method::PUT;
    const ROUTE: &'static str = "/move-document";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameDocumentRequest {
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
    InvalidUsername,
    NotPermissioned,
    UserNotFound,
    DocumentNotFound,
    DocumentDeleted,
    EditConflict,
    DocumentPathTaken,
}

impl RenameDocumentRequest {
    pub fn new(file_metadata: &FileMetadata) -> Self {
        RenameDocumentRequest {
            id: file_metadata.id,
            old_metadata_version: file_metadata.metadata_version,
            new_name: file_metadata.name.clone(),
        }
    }
}

impl Request for RenameDocumentRequest {
    type Response = RenameDocumentResponse;
    type Error = RenameDocumentError;
    const METHOD: Method = Method::PUT;
    const ROUTE: &'static str = "/rename-document";
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
    DocumentNotFound,
}

impl Request for GetDocumentRequest {
    type Response = GetDocumentResponse;
    type Error = GetDocumentError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-document";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CreateFolderRequest {
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
    InvalidUsername,
    NotPermissioned,
    UserNotFound,
    FileIdTaken,
    FolderPathTaken,
    ParentNotFound,
}

impl CreateFolderRequest {
    pub fn new(file_metadata: &FileMetadata) -> Self {
        CreateFolderRequest {
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
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/create-folder";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFolderRequest {
    pub id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteFolderResponse {
    pub new_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum DeleteFolderError {
    InvalidUsername,
    NotPermissioned,
    UserNotFound,
    FolderNotFound,
    EditConflict,
    FolderDeleted,
    CannotDeleteRoot,
    ClientUpdateRequired,
}

impl Request for DeleteFolderRequest {
    type Response = DeleteFolderResponse;
    type Error = DeleteFolderError;
    const METHOD: Method = Method::DELETE;
    const ROUTE: &'static str = "/delete-folder";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MoveFolderRequest {
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
    InvalidUsername,
    NotPermissioned,
    UserNotFound,
    FolderNotFound,
    ParentNotFound,
    ParentDeleted,
    CannotMoveRoot,
    CannotMoveIntoDescendant,
    EditConflict,
    FolderDeleted,
    FolderPathTaken,
}

impl MoveFolderRequest {
    pub fn new(file_metadata: &FileMetadata) -> Self {
        MoveFolderRequest {
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
    const METHOD: Method = Method::PUT;
    const ROUTE: &'static str = "/move-folder";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RenameFolderRequest {
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
    InvalidUsername,
    NotPermissioned,
    UserNotFound,
    FolderNotFound,
    FolderDeleted,
    CannotRenameRoot,
    EditConflict,
    FolderPathTaken,
}

impl RenameFolderRequest {
    pub fn new(file_metadata: &FileMetadata) -> Self {
        RenameFolderRequest {
            id: file_metadata.id,
            old_metadata_version: file_metadata.metadata_version,
            new_name: file_metadata.name.clone(),
        }
    }
}

impl Request for RenameFolderRequest {
    type Response = RenameFolderResponse;
    type Error = RenameFolderError;
    const METHOD: Method = Method::PUT;
    const ROUTE: &'static str = "/rename-folder";
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
    InvalidUsername,
    UserNotFound,
}

impl Request for GetPublicKeyRequest {
    type Response = GetPublicKeyResponse;
    type Error = GetPublicKeyError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-public-key";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUsageRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUsageResponse {
    pub usages: Vec<FileUsage>,
    pub cap: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FileUsage {
    pub file_id: Uuid,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetUsageError {
    InvalidUsername,
    UserNotFound,
}

impl Request for GetUsageRequest {
    type Response = GetUsageResponse;
    type Error = GetUsageError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-usage";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUpdatesRequest {
    pub since_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUpdatesResponse {
    pub file_metadata: Vec<FileMetadata>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetUpdatesError {
    UserNotFound,
    InvalidUsername,
}

impl Request for GetUpdatesRequest {
    type Response = GetUpdatesResponse;
    type Error = GetUpdatesError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-updates";
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
    UsernameTaken,
    InvalidPublicKey,
    InvalidUserAccessKey,
    InvalidUsername,
    FileIdTaken,
}

impl NewAccountRequest {
    pub fn new(account: &Account, root_metadata: &FileMetadata) -> Self {
        NewAccountRequest {
            username: account.username.clone(),
            public_key: account.private_key.to_public_key(),
            folder_id: root_metadata.id,
            parent_access_key: root_metadata.folder_access_keys.clone(),
            user_access_key: root_metadata
                .user_access_keys
                .get(&account.username)
                .expect("file metadata for new account request must have user access key") // TODO: handle better
                .access_key
                .clone(),
        }
    }
}

impl Request for NewAccountRequest {
    type Response = NewAccountResponse;
    type Error = NewAccountError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/new-account";
}
