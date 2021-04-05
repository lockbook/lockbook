use crate::service::code_version_service::CodeVersion;
use lockbook_crypto::crypto_service::{PubKeyCryptoService, RSASignError};
use lockbook_models::account::Account;
use lockbook_models::api::*;
use reqwest::blocking::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Debug)]
pub enum ApiError<E> {
    Endpoint(E),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
    Sign(RSASignError),
    Serialize(serde_json::error::Error),
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    Deserialize(serde_json::error::Error),
}

impl<E> From<ErrorWrapper<E>> for ApiError<E> {
    fn from(err: ErrorWrapper<E>) -> Self {
        match err {
            ErrorWrapper::Endpoint(e) => ApiError::Endpoint(e),
            ErrorWrapper::ClientUpdateRequired => ApiError::ClientUpdateRequired,
            ErrorWrapper::InvalidAuth => ApiError::InvalidAuth,
            ErrorWrapper::ExpiredAuth => ApiError::ExpiredAuth,
            ErrorWrapper::InternalError => ApiError::InternalError,
            ErrorWrapper::BadRequest => ApiError::BadRequest,
        }
    }
}

pub trait Client {
    fn request<
        T: Request<Response = impl DeserializeOwned, Error = impl DeserializeOwned> + Serialize,
    >(
        account: &Account,
        request: T,
    ) -> Result<T::Response, ApiError<T::Error>>;
}

pub struct ClientImpl<Crypto: PubKeyCryptoService, Version: CodeVersion> {
    _crypto: Crypto,
    _version: Version,
}

impl<Crypto: PubKeyCryptoService, Version: CodeVersion> Client for ClientImpl<Crypto, Version> {
    fn request<
        T: Request<Response = impl DeserializeOwned, Error = impl DeserializeOwned> + Serialize,
    >(
        account: &Account,
        request: T,
    ) -> Result<T::Response, ApiError<T::Error>> {
        let client = ReqwestClient::new();
        let signed_request = Crypto::sign(&account.private_key, request).map_err(ApiError::Sign)?;
        let serialized_request = serde_json::to_vec(&RequestWrapper {
            signed_request,
            client_version: String::from(Version::get_code_version()),
        })
        .map_err(ApiError::Serialize)?;
        let serialized_response = client
            .request(
                T::METHOD,
                format!("{}{}", account.api_url, T::ROUTE).as_str(),
            )
            .body(serialized_request)
            .send()
            .map_err(ApiError::SendFailed)?
            .bytes()
            .map_err(ApiError::ReceiveFailed)?;
        let response: Result<T::Response, ErrorWrapper<T::Error>> =
            serde_json::from_slice(&serialized_response).map_err(ApiError::Deserialize)?;
        response.map_err(ApiError::from)
    }
}
