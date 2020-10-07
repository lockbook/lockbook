use crate::model::api::*;
use crate::model::crypto::*;
use rsa::RSAPublicKey;

use crate::model::file_metadata::FileMetadata;
use crate::CORE_CODE_VERSION;
use reqwest::blocking::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug)]
pub enum Error<ApiError> {
    Serialize(serde_json::error::Error),
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    Deserialize(serde_json::error::Error),
    Api(ApiError),
}

pub fn api_request<Request: Serialize, Response: DeserializeOwned, ApiError: DeserializeOwned>(
    api_url: &str,
    method: Method,
    endpoint: &str,
    request: &Request,
) -> Result<Response, Error<ApiError>> {
    let client = ReqwestClient::new();
    let serialized_request = serde_json::to_string(&request).map_err(Error::Serialize)?;
    let serialized_response = client
        .request(method, format!("{}/{}", api_url, endpoint).as_str())
        .body(serialized_request)
        .send()
        .map_err(Error::SendFailed)?
        .text()
        .map_err(Error::ReceiveFailed)?;
    let response: Result<Response, ApiError> =
        serde_json::from_str(&serialized_response).map_err(Error::Deserialize)?;
    response.map_err(Error::Api)
}

pub trait Client {
    fn get_document(
        api_url: &str,
        id: Uuid,
        content_version: u64,
    ) -> Result<Document, Error<GetDocumentError>>;
    fn change_document_content(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_content: EncryptedValueWithNonce,
    ) -> Result<u64, Error<ChangeDocumentContentError>>;
    fn create_document(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        name: &str,
        parent: Uuid,
        content: EncryptedValueWithNonce,
        parent_access_key: FolderAccessInfo,
    ) -> Result<u64, Error<CreateDocumentError>>;
    fn delete_document(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteDocumentError>>;
    fn move_document(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
        new_folder_access: FolderAccessInfo,
    ) -> Result<u64, Error<MoveDocumentError>>;
    fn rename_document(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameDocumentError>>;
    fn create_folder(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        name: &str,
        parent: Uuid,
        parent_access_key: FolderAccessInfo,
    ) -> Result<u64, Error<CreateFolderError>>;
    fn delete_folder(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteFolderError>>;
    fn move_folder(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
        new_access_keys: FolderAccessInfo,
    ) -> Result<u64, Error<MoveFolderError>>;
    fn rename_folder(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameFolderError>>;
    fn get_public_key(
        api_url: &str,
        username: &str,
    ) -> Result<RSAPublicKey, Error<GetPublicKeyError>>;
    fn get_updates(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        since_metadata_version: u64,
    ) -> Result<Vec<FileMetadata>, Error<GetUpdatesError>>;
    fn new_account(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        public_key: RSAPublicKey,
        folder_id: Uuid,
        parent_access_key: FolderAccessInfo,
        user_access_key: EncryptedValue,
    ) -> Result<u64, Error<NewAccountError>>;
    fn get_usage(api_url: &str, username: &str) -> Result<GetUsageResponse, Error<GetUsageError>>;
}

pub struct ClientImpl;
impl Client for ClientImpl {
    fn get_document(
        api_url: &str,
        id: Uuid,
        content_version: u64,
    ) -> Result<Document, Error<GetDocumentError>> {
        api_request(
            api_url,
            Method::GET,
            "get-document",
            &GetDocumentRequest {
                id: id,
                client_version: CORE_CODE_VERSION.to_string(),
                content_version: content_version,
            },
        )
        .map(|r: GetDocumentResponse| Document { content: r.content })
    }
    fn change_document_content(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_content: EncryptedValueWithNonce,
    ) -> Result<u64, Error<ChangeDocumentContentError>> {
        api_request(
            api_url,
            Method::PUT,
            "change-document-content",
            &ChangeDocumentContentRequest {
                username: String::from(username),
                signature: signature.clone(),
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
        signature: &SignedValue,
        id: Uuid,
        name: &str,
        parent: Uuid,
        content: EncryptedValueWithNonce,
        parent_access_key: FolderAccessInfo,
    ) -> Result<u64, Error<CreateDocumentError>> {
        api_request(
            api_url,
            Method::POST,
            "create-document",
            &CreateDocumentRequest {
                username: String::from(username),
                signature: signature.clone(),
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
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteDocumentError>> {
        api_request(
            api_url,
            Method::DELETE,
            "delete-document",
            &DeleteDocumentRequest {
                username: String::from(username),
                signature: signature.clone(),
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
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
        new_folder_access: FolderAccessInfo,
    ) -> Result<u64, Error<MoveDocumentError>> {
        api_request(
            api_url,
            Method::PUT,
            "move-document",
            &MoveDocumentRequest {
                username: String::from(username),
                signature: signature.clone(),
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
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameDocumentError>> {
        api_request(
            api_url,
            Method::PUT,
            "rename-document",
            &RenameDocumentRequest {
                username: String::from(username),
                signature: signature.clone(),
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
        signature: &SignedValue,
        id: Uuid,
        name: &str,
        parent: Uuid,
        parent_access_key: FolderAccessInfo,
    ) -> Result<u64, Error<CreateFolderError>> {
        api_request(
            api_url,
            Method::POST,
            "create-folder",
            &CreateFolderRequest {
                username: String::from(username),
                signature: signature.clone(),
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
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteFolderError>> {
        api_request(
            api_url,
            Method::DELETE,
            "delete-folder",
            &DeleteFolderRequest {
                username: String::from(username),
                signature: signature.clone(),
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
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
        new_access_keys: FolderAccessInfo,
    ) -> Result<u64, Error<MoveFolderError>> {
        api_request(
            api_url,
            Method::PUT,
            "move-folder",
            &MoveFolderRequest {
                username: String::from(username),
                signature: signature.clone(),
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
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameFolderError>> {
        api_request(
            api_url,
            Method::PUT,
            "rename-folder",
            &RenameFolderRequest {
                username: String::from(username),
                signature: signature.clone(),
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
    ) -> Result<RSAPublicKey, Error<GetPublicKeyError>> {
        api_request(
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
        signature: &SignedValue,
        since_metadata_version: u64,
    ) -> Result<Vec<FileMetadata>, Error<GetUpdatesError>> {
        api_request(
            api_url,
            Method::GET,
            "get-updates",
            &GetUpdatesRequest {
                username: String::from(username),
                signature: signature.clone(),
                client_version: CORE_CODE_VERSION.to_string(),
                since_metadata_version: since_metadata_version,
            },
        )
        .map(|r: GetUpdatesResponse| r.file_metadata)
    }
    fn new_account(
        api_url: &str,
        username: &str,
        signature: &SignedValue,
        public_key: RSAPublicKey,
        folder_id: Uuid,
        parent_access_key: FolderAccessInfo,
        user_access_key: EncryptedValue,
    ) -> Result<u64, Error<NewAccountError>> {
        api_request(
            api_url,
            Method::POST,
            "new-account",
            &NewAccountRequest {
                username: String::from(username),
                signature: signature.clone(),
                client_version: CORE_CODE_VERSION.to_string(),
                public_key: public_key,
                folder_id: folder_id,
                parent_access_key: parent_access_key,
                user_access_key: user_access_key,
            },
        )
        .map(|r: NewAccountResponse| r.folder_metadata_version)
    }
    fn get_usage(api_url: &str, username: &str) -> Result<GetUsageResponse, Error<GetUsageError>> {
        api_request(
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
