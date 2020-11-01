use crate::model::api::*;
use crate::model::crypto::RSASigned;
use crate::service::crypto_service::{PubKeyCryptoService, RSASignError};
use crate::CORE_CODE_VERSION;
use reqwest::blocking::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
use reqwest::Method;
use rsa::RSAPrivateKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum ApiError<E> {
    Sign(RSASignError),
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RequestWrapper<T: Request> {
    pub signed_request: RSASigned<T>,
    pub client_version: String,
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
        "new-account"
    }
}

pub trait Client {
    fn request<T: Request>(
        api_url: &str,
        key: &RSAPrivateKey,
        request: T,
    ) -> Result<T::Response, ApiError<T::Error>>;
}

pub struct ClientImpl<Crypto: PubKeyCryptoService> {
    _crypto: Crypto,
}

impl<Crypto: PubKeyCryptoService> Client for ClientImpl<Crypto> {
    fn request<T: Request>(
        api_url: &str,
        key: &RSAPrivateKey,
        request: T,
    ) -> Result<T::Response, ApiError<T::Error>> {
        let client = ReqwestClient::new();
        let signed_request = Crypto::sign(key, request).map_err(ApiError::Sign)?;
        let serialized_request = serde_json::to_string(&RequestWrapper {
            signed_request,
            client_version: String::from(CORE_CODE_VERSION),
        })
        .map_err(ApiError::Serialize)?;
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
