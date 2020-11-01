use crate::model::api::*;
use crate::model::crypto::*;
use rsa::RSAPublicKey;

use crate::model::file_metadata::FileMetadata;
use crate::service::crypto_service::PubKeyCryptoService;
use crate::CORE_CODE_VERSION;
use reqwest::blocking::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug)]
pub enum ApiError<E> {
    Serialize(serde_json::error::Error),
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    Deserialize(serde_json::error::Error),
    Api(E),
}

// TODO: sign requests
pub trait Client {
    fn get_document(
        api_url: &str,
        id: Uuid,
        content_version: u64,
    ) -> Result<EncryptedDocument, ApiError<GetDocumentError>>;
    fn change_document_content(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_content: EncryptedDocument,
    ) -> Result<u64, ApiError<ChangeDocumentContentError>>;
    fn create_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        name: &str,
        parent: Uuid,
        content: EncryptedDocument,
        parent_access_key: FolderAccessInfo,
    ) -> Result<u64, ApiError<CreateDocumentError>>;
    fn delete_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, ApiError<DeleteDocumentError>>;
    fn move_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
        new_folder_access: FolderAccessInfo,
    ) -> Result<u64, ApiError<MoveDocumentError>>;
    fn rename_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, ApiError<RenameDocumentError>>;
    fn create_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        name: &str,
        parent: Uuid,
        parent_access_key: FolderAccessInfo,
    ) -> Result<u64, ApiError<CreateFolderError>>;
    fn delete_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, ApiError<DeleteFolderError>>;
    fn move_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
        new_access_keys: FolderAccessInfo,
    ) -> Result<u64, ApiError<MoveFolderError>>;
    fn rename_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, ApiError<RenameFolderError>>;
    fn get_public_key(
        api_url: &str,
        username: &str,
    ) -> Result<RSAPublicKey, ApiError<GetPublicKeyError>>;
    fn get_updates(
        api_url: &str,
        username: &str,
        since_metadata_version: u64,
    ) -> Result<Vec<FileMetadata>, ApiError<GetUpdatesError>>;
    fn new_account(
        api_url: &str,
        username: &str,
        public_key: RSAPublicKey,
        folder_id: Uuid,
        parent_access_key: FolderAccessInfo,
        user_access_key: EncryptedUserAccessKey,
    ) -> Result<u64, ApiError<NewAccountError>>;
    fn get_usage(
        api_url: &str,
        username: &str,
    ) -> Result<GetUsageResponse, ApiError<GetUsageError>>;
}

pub struct ClientImpl<Crypto: PubKeyCryptoService> {
    _crypto: Crypto,
}

impl<Crypto: PubKeyCryptoService> ClientImpl<Crypto> {
    pub fn api_request<Request: Serialize, Response: DeserializeOwned, E: DeserializeOwned>(
        api_url: &str,
        method: Method,
        endpoint: &str,
        request: &Request,
    ) -> Result<Response, ApiError<E>> {
        let client = ReqwestClient::new();
        let serialized_request = serde_json::to_string(&request).map_err(ApiError::Serialize)?;
        let serialized_response = client
            .request(method, format!("{}/{}", api_url, endpoint).as_str())
            .body(serialized_request)
            .send()
            .map_err(ApiError::SendFailed)?
            .text()
            .map_err(ApiError::ReceiveFailed)?;
        let response: Result<Response, E> =
            serde_json::from_str(&serialized_response).map_err(ApiError::Deserialize)?;
        response.map_err(ApiError::Api)
    }
}

impl<Crypto: PubKeyCryptoService> Client for ClientImpl<Crypto> {
    fn get_document(
        api_url: &str,
        id: Uuid,
        content_version: u64,
    ) -> Result<EncryptedDocument, ApiError<GetDocumentError>> {
        Self::api_request(
            api_url,
            Method::GET,
            "get-document",
            &GetDocumentRequest {
                id: id,
                client_version: CORE_CODE_VERSION.to_string(),
                content_version: content_version,
            },
        )
        .map(|r: GetDocumentResponse| r.content)
    }
    fn change_document_content(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_content: EncryptedDocument,
    ) -> Result<u64, ApiError<ChangeDocumentContentError>> {
        Self::api_request(
            api_url,
            Method::PUT,
            "change-document-content",
            &ChangeDocumentContentRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_content: new_content,
            },
        )
        .map(|r: ChangeDocumentContentResponse| r.new_metadata_and_content_version)
    }
    fn create_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        name: &str,
        parent: Uuid,
        content: EncryptedDocument,
        parent_access_key: FolderAccessInfo,
    ) -> Result<u64, ApiError<CreateDocumentError>> {
        Self::api_request(
            api_url,
            Method::POST,
            "create-document",
            &CreateDocumentRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                name: String::from(name),
                parent: parent,
                content: content,
                parent_access_key: parent_access_key,
            },
        )
        .map(|r: CreateDocumentResponse| r.new_metadata_and_content_version)
    }
    fn delete_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, ApiError<DeleteDocumentError>> {
        Self::api_request(
            api_url,
            Method::DELETE,
            "delete-document",
            &DeleteDocumentRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
            },
        )
        .map(|r: DeleteDocumentResponse| r.new_metadata_and_content_version)
    }
    fn move_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
        new_folder_access: FolderAccessInfo,
    ) -> Result<u64, ApiError<MoveDocumentError>> {
        Self::api_request(
            api_url,
            Method::PUT,
            "move-document",
            &MoveDocumentRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_parent: new_parent,
                new_folder_access,
            },
        )
        .map(|r: MoveDocumentResponse| r.new_metadata_version)
    }
    fn rename_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, ApiError<RenameDocumentError>> {
        Self::api_request(
            api_url,
            Method::PUT,
            "rename-document",
            &RenameDocumentRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_name: String::from(new_name),
            },
        )
        .map(|r: RenameDocumentResponse| r.new_metadata_version)
    }
    fn create_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        name: &str,
        parent: Uuid,
        parent_access_key: FolderAccessInfo,
    ) -> Result<u64, ApiError<CreateFolderError>> {
        Self::api_request(
            api_url,
            Method::POST,
            "create-folder",
            &CreateFolderRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                name: String::from(name),
                parent: parent,
                parent_access_key: parent_access_key,
            },
        )
        .map(|r: CreateFolderResponse| r.new_metadata_version)
    }
    fn delete_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, ApiError<DeleteFolderError>> {
        Self::api_request(
            api_url,
            Method::DELETE,
            "delete-folder",
            &DeleteFolderRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
            },
        )
        .map(|r: DeleteFolderResponse| r.new_metadata_version)
    }
    fn move_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
        new_access_keys: FolderAccessInfo,
    ) -> Result<u64, ApiError<MoveFolderError>> {
        Self::api_request(
            api_url,
            Method::PUT,
            "move-folder",
            &MoveFolderRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_parent: new_parent,
                new_folder_access: new_access_keys,
            },
        )
        .map(|r: MoveFolderResponse| r.new_metadata_version)
    }
    fn rename_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, ApiError<RenameFolderError>> {
        Self::api_request(
            api_url,
            Method::PUT,
            "rename-folder",
            &RenameFolderRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_name: String::from(new_name),
            },
        )
        .map(|r: RenameFolderResponse| r.new_metadata_version)
    }
    fn get_public_key(
        api_url: &str,
        username: &str,
    ) -> Result<RSAPublicKey, ApiError<GetPublicKeyError>> {
        Self::api_request(
            api_url,
            Method::GET,
            "get-public-key",
            &GetPublicKeyRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
            },
        )
        .map(|r: GetPublicKeyResponse| r.key)
    }
    fn get_updates(
        api_url: &str,
        username: &str,
        since_metadata_version: u64,
    ) -> Result<Vec<FileMetadata>, ApiError<GetUpdatesError>> {
        Self::api_request(
            api_url,
            Method::GET,
            "get-updates",
            &GetUpdatesRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                since_metadata_version: since_metadata_version,
            },
        )
        .map(|r: GetUpdatesResponse| r.file_metadata)
    }
    fn new_account(
        api_url: &str,
        username: &str,
        public_key: RSAPublicKey,
        folder_id: Uuid,
        parent_access_key: FolderAccessInfo,
        user_access_key: EncryptedUserAccessKey,
    ) -> Result<u64, ApiError<NewAccountError>> {
        Self::api_request(
            api_url,
            Method::POST,
            "new-account",
            &NewAccountRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                public_key: public_key,
                folder_id: folder_id,
                parent_access_key: parent_access_key,
                user_access_key: user_access_key,
            },
        )
        .map(|r: NewAccountResponse| r.folder_metadata_version)
    }
    fn get_usage(
        api_url: &str,
        username: &str,
    ) -> Result<GetUsageResponse, ApiError<GetUsageError>> {
        Self::api_request(
            api_url,
            Method::GET,
            "get-usage",
            &GetUsageRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
            },
        )
    }
}
