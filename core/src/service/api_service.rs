use reqwest::blocking::Client as RequestClient;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::get_code_version;
use lockbook_shared::account::Account;
use lockbook_shared::api::*;
use lockbook_shared::clock::{get_time, Timestamp};
use lockbook_shared::pubkey;

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

#[derive(Debug, PartialEq, Eq)]
pub enum ApiError<E> {
    Endpoint(E),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
    Sign(lockbook_shared::SharedError),
    Serialize(String),
    SendFailed(String),
    ReceiveFailed(String),
    Deserialize(String),
}

pub trait Requester {
    fn request<
        T: Request<Response = impl DeserializeOwned, Error = impl DeserializeOwned> + Serialize,
    >(
        &self, account: &Account, request: T,
    ) -> Result<T::Response, ApiError<T::Error>>;
}

#[derive(Debug, Clone)]
pub struct Network {
    pub client: RequestClient,
    pub get_code_version: fn() -> &'static str,
    pub get_time: fn() -> Timestamp,
}

impl Default for Network {
    fn default() -> Self {
        Self { client: Default::default(), get_code_version, get_time }
    }
}

impl Requester for Network {
    fn request<
        T: Request<Response = impl DeserializeOwned, Error = impl DeserializeOwned> + Serialize,
    >(
        &self, account: &Account, request: T,
    ) -> Result<T::Response, ApiError<T::Error>> {
        let signed_request =
            pubkey::sign(&account.private_key, request, self.get_time).map_err(ApiError::Sign)?;
        let serialized_request = serde_json::to_vec(&RequestWrapper {
            signed_request,
            client_version: String::from((self.get_code_version)()),
        })
        .map_err(|err| ApiError::Serialize(err.to_string()))?;
        let serialized_response = self
            .client
            .request(T::METHOD, format!("{}{}", account.api_url, T::ROUTE).as_str())
            .body(serialized_request)
            .send()
            .map_err(|err| {
                warn!("Send failed: {:#?}", err);
                ApiError::SendFailed(err.to_string())
            })?
            .bytes()
            .map_err(|err| ApiError::ReceiveFailed(err.to_string()))?;
        let response: Result<T::Response, ErrorWrapper<T::Error>> =
            serde_json::from_slice(&serialized_response)
                .map_err(|err| ApiError::Deserialize(err.to_string()))?;
        response.map_err(ApiError::from)
    }
}
