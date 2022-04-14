use reqwest::blocking::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::get_code_version;
use lockbook_crypto::clock_service::{get_time, Timestamp};
use lockbook_crypto::pubkey;
use lockbook_crypto::pubkey::ECSignError;
use lockbook_models::account::Account;
use lockbook_models::api::*;

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

#[derive(Debug)]
pub enum ApiError<E> {
    Endpoint(E),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
    Sign(ECSignError),
    Serialize(serde_json::error::Error),
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    Deserialize(serde_json::error::Error),
}

pub fn request<
    T: Request<Response = impl DeserializeOwned, Error = impl DeserializeOwned> + Serialize,
>(
    account: &Account, request: T,
) -> Result<T::Response, ApiError<T::Error>> {
    request_helper(account, request, get_code_version, get_time)
}

pub fn request_helper<
    T: Request<Response = impl DeserializeOwned, Error = impl DeserializeOwned> + Serialize,
>(
    account: &Account, request: T, get_code_version: fn() -> &'static str,
    get_time: fn() -> Timestamp,
) -> Result<T::Response, ApiError<T::Error>> {
    let client = ReqwestClient::new();
    let signed_request =
        pubkey::sign(&account.private_key, request, get_time).map_err(ApiError::Sign)?;
    let serialized_request = serde_json::to_vec(&RequestWrapper {
        signed_request,
        client_version: String::from(get_code_version()),
    })
    .map_err(ApiError::Serialize)?;
    let serialized_response = client
        .request(T::METHOD, format!("{}{}", account.api_url, T::ROUTE).as_str())
        .body(serialized_request)
        .send()
        .map_err(ApiError::SendFailed)?
        .bytes()
        .map_err(ApiError::ReceiveFailed)?;
    let response: Result<T::Response, ErrorWrapper<T::Error>> =
        serde_json::from_slice(&serialized_response).map_err(ApiError::Deserialize)?;
    response.map_err(ApiError::from)
}
