use web_time::{Duration, Instant};

#[cfg(not(target_family = "wasm"))]
use bytes::Bytes;
#[cfg(not(target_family = "wasm"))]
use futures::stream;
use reqwest::{Body, Client};

use crate::get_code_version;
use crate::model::account::Account;
use crate::model::api::*;
use crate::model::clock::{Timestamp, get_time};
use crate::model::errors::LbErr;
use crate::model::pubkey;
use crate::model::wire::{WIRE_FORMAT_HEADER, WireFormat};

const STREAM_CHUNK_BYTES: usize = 4 * 1024 * 1024;

const STREAM_BODY_THRESHOLD: usize = 1024 * 1024 * 1024;

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
    Sign(LbErr),
    Serialize(String),
    SendFailed(String),
    ReceiveFailed(String),
    Deserialize(String),
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Network {
    pub client: Client,
    pub get_code_version: fn() -> &'static str,
    pub get_time: fn() -> Timestamp,
}

impl Default for Network {
    fn default() -> Self {
        Self { client: Default::default(), get_code_version, get_time }
    }
}

impl Network {
    #[instrument(level = "debug", skip(self, account, request), fields(route=T::ROUTE), err(Debug))]
    pub async fn request<T: Request>(
        &self, account: &Account, request: T,
    ) -> Result<T::Response, ApiError<T::Error>> {
        let signed_request =
            pubkey::sign(&account.private_key, &account.public_key(), request, self.get_time)
                .map_err(ApiError::Sign)?;

        let client_version = String::from((self.get_code_version)());

        let wire_format = WireFormat::CLIENT_DEFAULT;
        let serialized_request = wire_format
            .serialize(&RequestWrapper { signed_request, client_version: client_version.clone() })
            .map_err(|err| ApiError::Serialize(err.to_string()))?;

        if serialized_request.len() > 10 * 1024 * 1024 {
            warn!(
                "making network request with {} bytes ({:?})",
                serialized_request.len(),
                wire_format
            );
        }

        let url = &account.api_url;
        let start = Instant::now();
        let body = body_for(serialized_request);
        let sent = self
            .client
            .request(T::METHOD, format!("{}{}", url, T::ROUTE).as_str())
            .body(body)
            .header("Accept-Version", client_version)
            .header(WIRE_FORMAT_HEADER, wire_format.as_str())
            .send()
            .await
            .map_err(|e| {
                warn!("send failed: {e:?}");
                ApiError::SendFailed(e.to_string())
            })?;
        if start.elapsed() > Duration::from_millis(1000) {
            warn!("network request took {:?}", start.elapsed());
        }

        let serialized_response = sent
            .bytes()
            .await
            .map_err(|err| ApiError::ReceiveFailed(err.to_string()))?;
        let response: Result<T::Response, ErrorWrapper<T::Error>> = wire_format
            .deserialize(&serialized_response)
            .map_err(|err| ApiError::Deserialize(err.to_string()))?;
        response.map_err(ApiError::from)
    }
}

#[cfg(not(target_family = "wasm"))]
fn body_for(serialized_request: Vec<u8>) -> Body {
    if serialized_request.len() < STREAM_BODY_THRESHOLD {
        return Body::from(serialized_request);
    }
    let mut buf = Bytes::from(serialized_request);
    let mut chunks: Vec<Result<Bytes, std::io::Error>> =
        Vec::with_capacity(buf.len().div_ceil(STREAM_CHUNK_BYTES));
    while !buf.is_empty() {
        let n = buf.len().min(STREAM_CHUNK_BYTES);
        chunks.push(Ok(buf.split_to(n)));
    }
    Body::wrap_stream(stream::iter(chunks))
}

#[cfg(target_family = "wasm")]
fn body_for(serialized_request: Vec<u8>) -> Body {
    Body::from(serialized_request)
}
