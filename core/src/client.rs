use crate::model::api::*;
use crate::model::aliases::*;
use crate::service::file_encryption_service::EncryptedFile;
use crate::{API_LOC, BUCKET_LOC};
use rsa::RSAPublicKey;

use reqwest::blocking::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
use serde::{Deserialize, Serialize};

pub enum Error<ApiError> {
    Serialize(serde_json::error::Error),
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    Deserialize(serde_json::error::Error),
    Api(ApiError),
}

pub fn api_request<'a, Request: Serialize, Response: Deserialize<'a>, ApiError: Deserialize<'a>>(endpoint: &str, request: &Request) -> Result<Response, Error<ApiError>> {
    let client = ReqwestClient::new();
    let serialized_request = serde_json::to_string(&request).map_err(Error::Serialize)?;
    let serialized_response = client
        .post(format!("{}/{}", API_LOC, endpoint).as_str())
        .body(serialized_request)
        .send()
        .map_err(Error::SendFailed)?
        .text()
        .map_err(Error::ReceiveFailed)?;
    let response: Result<Response, ApiError> = serde_json::from_str(&serialized_response).map_err(Error::Deserialize)?;
    response.map_err(Error::Api)
}

pub trait Client {
    fn get_document(
        id: FileId,
        content_version: Version,
    ) -> Result<EncryptedFile, Error<()>>;
    fn change_document_content(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
        new_content: EncryptedDocumentContent,
    ) -> Result<Version, Error<ChangeDocumentContentError>>;
    fn create_document(
        username: Username,
        signature: Signature,
        id: FileId,
        name: Filename,
        parent: FileId,
        content: EncryptedDocumentContent,
    ) -> Result<Version, Error<CreateDocumentError>>;
    fn delete_document(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
    ) -> Result<Version, Error<DeleteDocumentError>>;
    fn move_document(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
        new_parent: FileId,
    ) -> Result<Version, Error<MoveDocumentError>>;
    fn rename_document(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
        new_name: Filename,
    ) -> Result<Version, Error<RenameDocumentError>>;
    fn create_folder(
        username: Username,
        signature: Signature,
        id: FileId,
        name: Filename,
        parent: FileId,
    ) -> Result<Version, Error<CreateFolderError>>;
    fn delete_folder(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
    ) -> Result<Version, Error<DeleteFolderError>>;
    fn move_folder(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
        new_parent: FileId,
    ) -> Result<Version, Error<MoveFolderError>>;
    fn rename_folder(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
        new_name: Filename,
    ) -> Result<Version, Error<RenameFolderError>>;
    fn get_public_key(
        username: Username,
    ) -> Result<RSAPublicKey, Error<GetPublicKeyError>>;
    fn get_updates(
        username: Username,
        signature: Signature,
        since_metadata_version: Version,
    ) -> Result<Vec<FileMetadata>, Error<GetUpdatesError>>;
    fn new_account(
        username: Username,
        signature: Signature,
        public_key: RSAPublicKey,
    ) -> Result<(), Error<NewAccountError>>;
}

pub struct ClientImpl;
impl Client for ClientImpl {
    fn get_document(
        id: FileId,
        content_version: Version,
    ) -> Result<EncryptedFile, Error<()>> {
        let client = ReqwestClient::new();
        let response = client
            .get(&format!("{}/{}-{}", BUCKET_LOC, id, content_version))
            .send()
            .map_err(Error::SendFailed)?;
        let response_body = response.text().map_err(Error::ReceiveFailed)?;
        let encrypted_file: EncryptedFile =
            serde_json::from_str(response_body.as_str()).map_err(Error::Deserialize)?;
        match response.status().as_u16() {
            200..=299 => Ok(encrypted_file),
            _ => Err(Error::Api(())),
        }
    }
    fn change_document_content(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
        new_content: EncryptedDocumentContent,
    ) -> Result<Version, Error<ChangeDocumentContentError>> {
        api_request(
            "",
            &ChangeDocumentContentRequest{
                username: username,
                signature: signature,
                id: id,
                old_metadata_version: old_metadata_version,
                new_content: new_content,
            },
        ).map(|r: ChangeDocumentContentResponse| r.new_metadata_and_content_version)
    }
    fn create_document(
        username: Username,
        signature: Signature,
        id: FileId,
        name: Filename,
        parent: FileId,
        content: EncryptedDocumentContent,
    ) -> Result<Version, Error<CreateDocumentError>> {
        api_request(
            "",
            &CreateDocumentRequest{
                username: username,
                signature: signature,
                id: id,
                name: name,
                parent: parent,
                content: content,
            },
        ).map(|r: CreateDocumentResponse| r.new_metadata_and_content_version)
    }
    fn delete_document(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
    ) -> Result<Version, Error<DeleteDocumentError>> {
        api_request(
            "",
            &DeleteDocumentRequest{
                username: username,
                signature: signature,
                id: id,
                old_metadata_version: old_metadata_version,
            },
        ).map(|r: DeleteDocumentResponse| r.new_metadata_and_content_version)
    }
    fn move_document(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
        new_parent: FileId,
    ) -> Result<Version, Error<MoveDocumentError>> {
        api_request(
            "",
            &MoveDocumentRequest{
                username: username,
                signature: signature,
                id: id,
                old_metadata_version: old_metadata_version,
                new_parent: new_parent,
            },
        ).map(|r: MoveDocumentResponse| r.new_metadata_version)
    }
    fn rename_document(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
        new_name: Filename,
    ) -> Result<Version, Error<RenameDocumentError>> {
        api_request(
            "",
            &RenameDocumentRequest{
                username: username,
                signature: signature,
                id: id,
                old_metadata_version: old_metadata_version,
                new_name: new_name,
            },
        ).map(|r: RenameDocumentResponse| r.new_metadata_version)
    }
    fn create_folder(
        username: Username,
        signature: Signature,
        id: FileId,
        name: Filename,
        parent: FileId,
    ) -> Result<Version, Error<CreateFolderError>> {
        api_request(
            "",
            &CreateFolderRequest{
                username: username,
                signature: signature,
                id: id,
                name: name,
                parent: parent,
            },
        ).map(|r: CreateFolderResponse| r.new_metadata_version)
    }
    fn delete_folder(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
    ) -> Result<Version, Error<DeleteFolderError>> {
        api_request(
            "",
            &DeleteFolderRequest{
                username: username,
                signature: signature,
                id: id,
                old_metadata_version: old_metadata_version,
            },
        ).map(|r: DeleteFolderResponse| r.new_metadata_version)
    }
    fn move_folder(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
        new_parent: FileId,
    ) -> Result<Version, Error<MoveFolderError>> {
        api_request(
            "",
            &MoveFolderRequest{
                username: username,
                signature: signature,
                id: id,
                old_metadata_version: old_metadata_version,
                new_parent: new_parent,
            },
        ).map(|r: MoveFolderResponse| r.new_metadata_version)
    }
    fn rename_folder(
        username: Username,
        signature: Signature,
        id: FileId,
        old_metadata_version: Version,
        new_name: Filename,
    ) -> Result<Version, Error<RenameFolderError>> {
        api_request(
            "",
            &RenameFolderRequest{
                username: username,
                signature: signature,
                id: id,
                old_metadata_version: old_metadata_version,
                new_name: new_name,
            },
        ).map(|r: RenameFolderResponse| r.new_metadata_version)
    }
    fn get_public_key(
        username: Username,
    ) -> Result<RSAPublicKey, Error<GetPublicKeyError>> {
        api_request(
            "",
            &GetPublicKeyRequest{
                username: username,
            }
        ).map(|r: GetPublicKeyResponse| r.key)
    }
    fn get_updates(
        username: Username,
        signature: Signature,
        since_metadata_version: Version,
    ) -> Result<Vec<FileMetadata>, Error<GetUpdatesError>> {
        api_request(
            "",
            &GetUpdatesRequest{
                username: username,
                signature: signature,
                since_metadata_version: since_metadata_version,
            }
        ).map(|r: GetUpdatesResponse| r.file_metadata)
    }
    fn new_account(
        username: Username,
        signature: Signature,
        public_key: RSAPublicKey,
    ) -> Result<(), Error<NewAccountError>> {
        api_request(
            "",
            &NewAccountRequest{
                username: username,
                signature: signature,
                public_key: public_key,
            }
        ).map(|r: NewAccountResponse| ())
    }
}
