use crate::model::api::*;
use crate::model::crypto::*;
use crate::{API_LOC, BUCKET_LOC};
use rsa::RSAPublicKey;

use reqwest::blocking::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
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

pub fn api_request<
    'a,
    Request: Serialize,
    Response: DeserializeOwned,
    ApiError: DeserializeOwned,
>(
    endpoint: &str,
    request: &Request,
) -> Result<Response, Error<ApiError>> {
    let client = ReqwestClient::new();
    let serialized_request = serde_json::to_string(&request).map_err(Error::Serialize)?;
    let serialized_response = client
        .post(format!("{}/{}", API_LOC, endpoint).as_str())
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
    fn get_document(id: Uuid, content_version: u64) -> Result<EncryptedFile, Error<()>>;
    fn change_document_content(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_content: EncryptedValueWithNonce,
    ) -> Result<u64, Error<ChangeDocumentContentError>>;
    fn create_document(
        username: &str,
        signature: &str,
        id: Uuid,
        name: &str,
        parent: Uuid,
        content: EncryptedValueWithNonce,
    ) -> Result<u64, Error<CreateDocumentError>>;
    fn delete_document(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteDocumentError>>;
    fn move_document(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
    ) -> Result<u64, Error<MoveDocumentError>>;
    fn rename_document(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameDocumentError>>;
    fn create_folder(
        username: &str,
        signature: &str,
        id: Uuid,
        name: &str,
        parent: Uuid,
    ) -> Result<u64, Error<CreateFolderError>>;
    fn delete_folder(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteFolderError>>;
    fn move_folder(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
    ) -> Result<u64, Error<MoveFolderError>>;
    fn rename_folder(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameFolderError>>;
    fn get_public_key(username: &str) -> Result<RSAPublicKey, Error<GetPublicKeyError>>;
    fn get_updates(
        username: &str,
        signature: &str,
        since_metadata_version: u64,
    ) -> Result<Vec<FileMetadata>, Error<GetUpdatesError>>;
    fn new_account(
        username: &str,
        signature: &str,
        public_key: RSAPublicKey,
    ) -> Result<(), Error<NewAccountError>>;
}

pub struct ClientImpl;
impl Client for ClientImpl {
    fn get_document(id: Uuid, content_version: u64) -> Result<EncryptedFile, Error<()>> {
        let client = ReqwestClient::new();
        let response = client
            .get(&format!("{}/{}-{}", BUCKET_LOC, id, content_version))
            .send()
            .map_err(Error::SendFailed)?;
        let status = response.status().as_u16();
        let response_body = response.text().map_err(Error::ReceiveFailed)?;
        let encrypted_file: EncryptedFile =
            serde_json::from_str(response_body.as_str()).map_err(Error::Deserialize)?;
        match status {
            200..=299 => Ok(encrypted_file),
            _ => Err(Error::Api(())),
        }
    }
    fn change_document_content(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_content: EncryptedValueWithNonce,
    ) -> Result<u64, Error<ChangeDocumentContentError>> {
        api_request(
            "change-document-content",
            &ChangeDocumentContentRequest {
                username: String::from(username),
                signature: String::from(signature),
                id: id,
                old_metadata_version: old_metadata_version,
                new_content: new_content,
            },
        )
        .map(|r: ChangeDocumentContentResponse| r.new_metadata_and_content_version)
    }
    fn create_document(
        username: &str,
        signature: &str,
        id: Uuid,
        name: &str,
        parent: Uuid,
        content: EncryptedValueWithNonce,
    ) -> Result<u64, Error<CreateDocumentError>> {
        api_request(
            "create-document",
            &CreateDocumentRequest {
                username: String::from(username),
                signature: String::from(signature),
                id: id,
                name: String::from(name),
                parent: parent,
                content: content,
            },
        )
        .map(|r: CreateDocumentResponse| r.new_metadata_and_content_version)
    }
    fn delete_document(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteDocumentError>> {
        api_request(
            "delete-document",
            &DeleteDocumentRequest {
                username: String::from(username),
                signature: String::from(signature),
                id: id,
                old_metadata_version: old_metadata_version,
            },
        )
        .map(|r: DeleteDocumentResponse| r.new_metadata_and_content_version)
    }
    fn move_document(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
    ) -> Result<u64, Error<MoveDocumentError>> {
        api_request(
            "move-document",
            &MoveDocumentRequest {
                username: String::from(username),
                signature: String::from(signature),
                id: id,
                old_metadata_version: old_metadata_version,
                new_parent: new_parent,
            },
        )
        .map(|r: MoveDocumentResponse| r.new_metadata_version)
    }
    fn rename_document(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameDocumentError>> {
        api_request(
            "rename-document",
            &RenameDocumentRequest {
                username: String::from(username),
                signature: String::from(signature),
                id: id,
                old_metadata_version: old_metadata_version,
                new_name: String::from(new_name),
            },
        )
        .map(|r: RenameDocumentResponse| r.new_metadata_version)
    }
    fn create_folder(
        username: &str,
        signature: &str,
        id: Uuid,
        name: &str,
        parent: Uuid,
    ) -> Result<u64, Error<CreateFolderError>> {
        api_request(
            "create-folder",
            &CreateFolderRequest {
                username: String::from(username),
                signature: String::from(signature),
                id: id,
                name: String::from(name),
                parent: parent,
            },
        )
        .map(|r: CreateFolderResponse| r.new_metadata_version)
    }
    fn delete_folder(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
    ) -> Result<u64, Error<DeleteFolderError>> {
        api_request(
            "delete-folder",
            &DeleteFolderRequest {
                username: String::from(username),
                signature: String::from(signature),
                id: id,
                old_metadata_version: old_metadata_version,
            },
        )
        .map(|r: DeleteFolderResponse| r.new_metadata_version)
    }
    fn move_folder(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_parent: Uuid,
    ) -> Result<u64, Error<MoveFolderError>> {
        api_request(
            "move-folder",
            &MoveFolderRequest {
                username: String::from(username),
                signature: String::from(signature),
                id: id,
                old_metadata_version: old_metadata_version,
                new_parent: new_parent,
            },
        )
        .map(|r: MoveFolderResponse| r.new_metadata_version)
    }
    fn rename_folder(
        username: &str,
        signature: &str,
        id: Uuid,
        old_metadata_version: u64,
        new_name: &str,
    ) -> Result<u64, Error<RenameFolderError>> {
        api_request(
            "rename-folder",
            &RenameFolderRequest {
                username: String::from(username),
                signature: String::from(signature),
                id: id,
                old_metadata_version: old_metadata_version,
                new_name: String::from(new_name),
            },
        )
        .map(|r: RenameFolderResponse| r.new_metadata_version)
    }
    fn get_public_key(username: &str) -> Result<RSAPublicKey, Error<GetPublicKeyError>> {
        api_request(
            "get-public-key",
            &GetPublicKeyRequest {
                username: String::from(username),
            },
        )
        .map(|r: GetPublicKeyResponse| r.key)
    }
    fn get_updates(
        username: &str,
        signature: &str,
        since_metadata_version: u64,
    ) -> Result<Vec<FileMetadata>, Error<GetUpdatesError>> {
        api_request(
            "get-updates",
            &GetUpdatesRequest {
                username: String::from(username),
                signature: String::from(signature),
                since_metadata_version: since_metadata_version,
            },
        )
        .map(|r: GetUpdatesResponse| r.file_metadata)
    }
    fn new_account(
        username: &str,
        signature: &str,
        public_key: RSAPublicKey,
    ) -> Result<(), Error<NewAccountError>> {
        api_request(
            "new-account",
            &NewAccountRequest {
                username: String::from(username),
                signature: String::from(signature),
                public_key: public_key,
            },
        )
        .map(|_r: NewAccountResponse| ())
    }
}
