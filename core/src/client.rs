use crate::model::api::*;
use crate::model::crypto::RSASigned;
use crate::service::code_version_service::CodeVersion;
use crate::service::crypto_service::{PubKeyCryptoService, RSASignError};
use reqwest::blocking::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
use rsa::RSAPrivateKey;
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RequestWrapper<T: Request> {
    pub signed_request: RSASigned<T>,
    pub client_version: String,
}

pub trait Client {
    fn request<T: Request>(
        api_url: &str,
        key: &RSAPrivateKey,
        request: T,
    ) -> Result<T::Response, ApiError<T::Error>>;
}

pub struct ClientImpl<Crypto: PubKeyCryptoService, Version: CodeVersion> {
    _crypto: Crypto,
    _version: Version,
}

impl<Crypto: PubKeyCryptoService, Version: CodeVersion> Client for ClientImpl<Crypto, Version> {
    fn request<T: Request>(
        api_url: &str,
        key: &RSAPrivateKey,
        request: T,
    ) -> Result<T::Response, ApiError<T::Error>> {
        let client = ReqwestClient::new();
        let signed_request = Crypto::sign(key, request).map_err(ApiError::Sign)?;
        let serialized_request = serde_json::to_string(&RequestWrapper {
            signed_request,
            client_version: String::from(Version::get_code_version()),
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
