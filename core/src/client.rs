use crate::model::api::*;
use crate::model::crypto::*;
use crate::{API_LOC, BUCKET_LOC};
use rsa::RSAPublicKey;

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
    api_location: &str,
    method: Method,
    endpoint: &str,
    request: &Request,
) -> Result<Response, Error<ApiError>> {
    let client = ReqwestClient::new();
    let serialized_request = serde_json::to_string(&request).map_err(Error::Serialize)?;
    let serialized_response = client
        .request(method, format!("{}/{}", api_location, endpoint).as_str())
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
    fn get_document(id: Uuid, content_version: u64) -> Result<Document, Error<()>>;
    fn change_document_content(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_content: EncryptedValueWithNonce,
    ) -> Result<u64, Error<ChangeDocumentContentError>>;
    fn create_document(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        name: &str,
        parent: Uuid,
        content: EncryptedValueWithNonce,
        parent_access_key: EncryptedValueWithNonce,
    ) -> Result<u64, Error<CreateDocumentError>>;
    fn delete_document(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteDocumentError>>;
    fn move_document(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
    ) -> Result<u64, Error<MoveDocumentError>>;
    fn rename_document(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameDocumentError>>;
    fn create_folder(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        name: &str,
        parent: Uuid,
        parent_access_key: EncryptedValueWithNonce,
    ) -> Result<u64, Error<CreateFolderError>>;
    fn delete_folder(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteFolderError>>;
    fn move_folder(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
    ) -> Result<u64, Error<MoveFolderError>>;
    fn rename_folder(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameFolderError>>;
    fn get_public_key(username: &str) -> Result<RSAPublicKey, Error<GetPublicKeyError>>;
    fn get_updates(
        username: &str,
        signature: &SignedValue,
        since_metadata_version: u64,
    ) -> Result<Vec<FileMetadata>, Error<GetUpdatesError>>;
    fn new_account(
        username: &str,
        signature: &SignedValue,
        public_key: RSAPublicKey,
        folder_id: Uuid,
        parent_access_key: EncryptedValueWithNonce,
        user_access_key: EncryptedValue,
    ) -> Result<u64, Error<NewAccountError>>;
}

pub struct ClientImpl;
impl Client for ClientImpl {
    fn get_document(id: Uuid, content_version: u64) -> Result<Document, Error<()>> {
        let client = ReqwestClient::new();
        let response = client
            .get(&format!("{}/{}-{}", BUCKET_LOC, id, content_version))
            .send()
            .map_err(Error::SendFailed)?;
        let status = response.status().as_u16();
        let response_body = response.text().map_err(Error::ReceiveFailed)?;
        let encrypted_file: Document = Document {
            content: serde_json::from_str(response_body.as_str()).map_err(Error::Deserialize)?,
        };
        match status {
            200..=299 => Ok(encrypted_file),
            _ => Err(Error::Api(())),
        }
    }
    fn change_document_content(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_content: EncryptedValueWithNonce,
    ) -> Result<u64, Error<ChangeDocumentContentError>> {
        api_request(
            API_LOC,
            Method::PUT,
            "change-document-content",
            &ChangeDocumentContentRequest {
                username: String::from(username),
                signature: signature.clone(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_content: new_content,
            },
        )
        .map(|r: ChangeDocumentContentResponse| r.new_metadata_and_content_version)
    }
    fn create_document(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        name: &str,
        parent: Uuid,
        content: EncryptedValueWithNonce,
        parent_access_key: EncryptedValueWithNonce,
    ) -> Result<u64, Error<CreateDocumentError>> {
        api_request(
            API_LOC,
            Method::POST,
            "create-document",
            &CreateDocumentRequest {
                username: String::from(username),
                signature: signature.clone(),
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
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteDocumentError>> {
        api_request(
            API_LOC,
            Method::DELETE,
            "delete-document",
            &DeleteDocumentRequest {
                username: String::from(username),
                signature: signature.clone(),
                id: id,
                old_metadata_version: old_metadata_version,
            },
        )
        .map(|r: DeleteDocumentResponse| r.new_metadata_and_content_version)
    }
    fn move_document(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
    ) -> Result<u64, Error<MoveDocumentError>> {
        api_request(
            API_LOC,
            Method::PUT,
            "move-document",
            &MoveDocumentRequest {
                username: String::from(username),
                signature: signature.clone(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_parent: new_parent,
            },
        )
        .map(|r: MoveDocumentResponse| r.new_metadata_version)
    }
    fn rename_document(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameDocumentError>> {
        api_request(
            API_LOC,
            Method::PUT,
            "rename-document",
            &RenameDocumentRequest {
                username: String::from(username),
                signature: signature.clone(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_name: String::from(new_name),
            },
        )
        .map(|r: RenameDocumentResponse| r.new_metadata_version)
    }
    fn create_folder(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        name: &str,
        parent: Uuid,
        parent_access_key: EncryptedValueWithNonce,
    ) -> Result<u64, Error<CreateFolderError>> {
        api_request(
            API_LOC,
            Method::POST,
            "create-folder",
            &CreateFolderRequest {
                username: String::from(username),
                signature: signature.clone(),
                id: id,
                name: String::from(name),
                parent: parent,
                parent_access_key: parent_access_key,
            },
        )
        .map(|r: CreateFolderResponse| r.new_metadata_version)
    }
    fn delete_folder(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteFolderError>> {
        api_request(
            API_LOC,
            Method::DELETE,
            "delete-folder",
            &DeleteFolderRequest {
                username: String::from(username),
                signature: signature.clone(),
                id: id,
                old_metadata_version: old_metadata_version,
            },
        )
        .map(|r: DeleteFolderResponse| r.new_metadata_version)
    }
    fn move_folder(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
    ) -> Result<u64, Error<MoveFolderError>> {
        api_request(
            API_LOC,
            Method::PUT,
            "move-folder",
            &MoveFolderRequest {
                username: String::from(username),
                signature: signature.clone(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_parent: new_parent,
            },
        )
        .map(|r: MoveFolderResponse| r.new_metadata_version)
    }
    fn rename_folder(
        username: &str,
        signature: &SignedValue,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameFolderError>> {
        api_request(
            API_LOC,
            Method::PUT,
            "rename-folder",
            &RenameFolderRequest {
                username: String::from(username),
                signature: signature.clone(),
                id: id,
                old_metadata_version: old_metadata_version,
                new_name: String::from(new_name),
            },
        )
        .map(|r: RenameFolderResponse| r.new_metadata_version)
    }
    fn get_public_key(username: &str) -> Result<RSAPublicKey, Error<GetPublicKeyError>> {
        api_request(
            API_LOC,
            Method::GET,
            "get-public-key",
            &GetPublicKeyRequest {
                username: String::from(username),
            },
        )
        .map(|r: GetPublicKeyResponse| r.key)
    }
    fn get_updates(
        username: &str,
        signature: &SignedValue,
        since_metadata_version: u64,
    ) -> Result<Vec<FileMetadata>, Error<GetUpdatesError>> {
        api_request(
            API_LOC,
            Method::GET,
            "get-updates",
            &GetUpdatesRequest {
                username: String::from(username),
                signature: signature.clone(),
                since_metadata_version: since_metadata_version,
            },
        )
        .map(|r: GetUpdatesResponse| r.file_metadata)
    }
    fn new_account(
        username: &str,
        signature: &SignedValue,
        public_key: RSAPublicKey,
        folder_id: Uuid,
        parent_access_key: EncryptedValueWithNonce,
        user_access_key: EncryptedValue,
    ) -> Result<u64, Error<NewAccountError>> {
        api_request(
            API_LOC,
            Method::POST,
            "new-account",
            &NewAccountRequest {
                username: String::from(username),
                signature: signature.clone(),
                public_key: public_key,
                folder_id: folder_id,
                parent_access_key: parent_access_key,
                user_access_key: user_access_key,
            },
        )
        .map(|r: NewAccountResponse| r.folder_metadata_version)
    }
}
