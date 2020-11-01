use crate::model::api::*;
use crate::model::crypto::*;
use crate::model::file_metadata::FileMetadata;
use crate::service::crypto_service::PubKeyCryptoService;
use crate::CORE_CODE_VERSION;
use reqwest::blocking::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
use reqwest::Method;
use rsa::RSAPublicKey;
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

pub trait Request: Serialize {
    type Response: DeserializeOwned;
    type Error: DeserializeOwned;
    fn method() -> Method;
    fn endpoint() -> &'static str;
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

impl Request for NewAccountRequest {
    type Response = NewAccountResponse;
    type Error = NewAccountError;
    fn method() -> Method {
        Method::POST
    }
    fn endpoint() -> &'static str {
        ""
    }
}

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
    pub fn request<T: Request>(
        api_url: &str,
        request: &T,
    ) -> Result<T::Response, ApiError<T::Error>> {
        let client = ReqwestClient::new();
        let serialized_request = serde_json::to_string(&request).map_err(ApiError::Serialize)?;
        let serialized_response = client
            .request(
                T::method(),
                format!("{}/{}", api_url, T::endpoint()).as_str(),
            )
            .body(serialized_request)
            .send()
            .map_err(ApiError::SendFailed)?
            .text()
            .map_err(ApiError::ReceiveFailed)?;
        let response: Result<T::Response, T::Error> =
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
        Self::request(
            api_url,
            &GetDocumentRequest {
                id: id,
                client_version: CORE_CODE_VERSION.to_string(),
                content_version: content_version,
            },
        )
        .map(|r| r.content)
    }
    fn change_document_content(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_content: EncryptedDocument,
    ) -> Result<u64, ApiError<ChangeDocumentContentError>> {
        Self::request(
            api_url,
            &ChangeDocumentContentRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_content: new_content,
            },
        )
        .map(|r| r.new_metadata_and_content_version)
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
        Self::request(
            api_url,
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
        .map(|r| r.new_metadata_and_content_version)
    }
    fn delete_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, ApiError<DeleteDocumentError>> {
        Self::request(
            api_url,
            &DeleteDocumentRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
            },
        )
        .map(|r| r.new_metadata_and_content_version)
    }
    fn move_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
        new_folder_access: FolderAccessInfo,
    ) -> Result<u64, ApiError<MoveDocumentError>> {
        Self::request(
            api_url,
            &MoveDocumentRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_parent: new_parent,
                new_folder_access,
            },
        )
        .map(|r| r.new_metadata_version)
    }
    fn rename_document(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, ApiError<RenameDocumentError>> {
        Self::request(
            api_url,
            &RenameDocumentRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_name: String::from(new_name),
            },
        )
        .map(|r| r.new_metadata_version)
    }
    fn create_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        name: &str,
        parent: Uuid,
        parent_access_key: FolderAccessInfo,
    ) -> Result<u64, ApiError<CreateFolderError>> {
        Self::request(
            api_url,
            &CreateFolderRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                name: String::from(name),
                parent: parent,
                parent_access_key: parent_access_key,
            },
        )
        .map(|r| r.new_metadata_version)
    }
    fn delete_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, ApiError<DeleteFolderError>> {
        Self::request(
            api_url,
            &DeleteFolderRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
            },
        )
        .map(|r| r.new_metadata_version)
    }
    fn move_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
        new_access_keys: FolderAccessInfo,
    ) -> Result<u64, ApiError<MoveFolderError>> {
        Self::request(
            api_url,
            &MoveFolderRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_parent: new_parent,
                new_folder_access: new_access_keys,
            },
        )
        .map(|r| r.new_metadata_version)
    }
    fn rename_folder(
        api_url: &str,
        username: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, ApiError<RenameFolderError>> {
        Self::request(
            api_url,
            &RenameFolderRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_name: String::from(new_name),
            },
        )
        .map(|r| r.new_metadata_version)
    }
    fn get_public_key(
        api_url: &str,
        username: &str,
    ) -> Result<RSAPublicKey, ApiError<GetPublicKeyError>> {
        Self::request(
            api_url,
            &GetPublicKeyRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
            },
        )
        .map(|r| r.key)
    }
    fn get_updates(
        api_url: &str,
        username: &str,
        since_metadata_version: u64,
    ) -> Result<Vec<FileMetadata>, ApiError<GetUpdatesError>> {
        Self::request(
            api_url,
            &GetUpdatesRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                since_metadata_version: since_metadata_version,
            },
        )
        .map(|r| r.file_metadata)
    }
    fn new_account(
        api_url: &str,
        username: &str,
        public_key: RSAPublicKey,
        folder_id: Uuid,
        parent_access_key: FolderAccessInfo,
        user_access_key: EncryptedUserAccessKey,
    ) -> Result<u64, ApiError<NewAccountError>> {
        Self::request(
            api_url,
            &NewAccountRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
                public_key: public_key,
                folder_id: folder_id,
                parent_access_key: parent_access_key,
                user_access_key: user_access_key,
            },
        )
        .map(|r| r.folder_metadata_version)
    }
    fn get_usage(
        api_url: &str,
        username: &str,
    ) -> Result<GetUsageResponse, ApiError<GetUsageError>> {
        Self::request(
            api_url,
            &GetUsageRequest {
                username: String::from(username),
                client_version: CORE_CODE_VERSION.to_string(),
            },
        )
    }
}
