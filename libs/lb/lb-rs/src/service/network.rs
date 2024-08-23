use reqwest::Client;

use crate::get_code_version;
use crate::logic::account::Account;
use crate::logic::api::*;
use crate::logic::clock::{get_time, Timestamp};
use crate::logic::pubkey;

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

// #[derive(Debug, PartialEq, Eq)]
#[derive(Debug)]
pub enum ApiError<E> {
    Endpoint(E),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
    Sign(crate::logic::SharedError),
    Serialize(String),
    SendFailed(String),
    ReceiveFailed(String),
    Deserialize(String),
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Network {
    pub(crate) client: Client,
    pub(crate) get_code_version: fn() -> &'static str,
    pub(crate) get_time: fn() -> Timestamp,
}

impl Default for Network {
    fn default() -> Self {
        Self { client: Default::default(), get_code_version, get_time }
    }
}

impl Network {
    pub(crate) async fn request<T: Request>(
        &self, account: &Account, request: T,
    ) -> Result<T::Response, ApiError<T::Error>> {
        let signed_request =
            pubkey::sign(&account.private_key, request, self.get_time).map_err(ApiError::Sign)?;

        let client_version = String::from((self.get_code_version)());

        let serialized_request = serde_json::to_vec(&RequestWrapper {
            signed_request,
            client_version: client_version.clone(),
        })
        .map_err(|err| ApiError::Serialize(err.to_string()))?;
        let a = self
            .client
            .request(T::METHOD, format!("{}{}", account.api_url, T::ROUTE).as_str())
            .body(serialized_request)
            .header("Accept-Version", client_version)
            .send()
            .await
            .map_err(|err| {
                warn!("Send failed: {:#?}", err);
                ApiError::SendFailed(err.to_string())
            })?;
        let serialized_response = a
            .bytes()
            .await
            .map_err(|err| ApiError::ReceiveFailed(err.to_string()))?;
        let response: Result<T::Response, ErrorWrapper<T::Error>> =
            serde_json::from_slice(&serialized_response)
                .map_err(|err| ApiError::Deserialize(err.to_string()))?;
        response.map_err(ApiError::from)
    }
}
