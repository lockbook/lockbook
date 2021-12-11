use crate::{account_service, file_service, get_build_info, metrics, RequestContext};
use crate::{verify_auth, verify_client_version, ServerError, ServerState};
use hyper::body::Bytes;
use hyper::{body, Body, Response, StatusCode};
use lazy_static::lazy_static;
use libsecp256k1::PublicKey;
use lockbook_crypto::pubkey::ECVerifyError;
use lockbook_models::api::*;
use log::*;
use prometheus::{register_histogram_vec, HistogramVec};
use serde::de::DeserializeOwned;
use serde::Serialize;

lazy_static! {
    static ref HTTP_REQUEST_DURATION_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "lockbook_server_request_duration_seconds",
        "The lockbook server's HTTP requests duration in seconds.",
        &["request"]
    )
    .unwrap();
}

macro_rules! route_case {
    ($TRequest:ty) => {
        (&<$TRequest>::METHOD, <$TRequest>::ROUTE)
    };
}

macro_rules! route_handler {
    ($TRequest:ty, $handler:path, $hyper_request:ident, $server_state: ident) => {{
        info!(
            "Request matched {}{}",
            <$TRequest>::METHOD,
            <$TRequest>::ROUTE
        );

        pack::<$TRequest>(match unpack(&$server_state, $hyper_request).await {
            Ok((request, public_key)) => {
                let request_string = format!("{:?}", request);
                let timer = HTTP_REQUEST_DURATION_HISTOGRAM
                    .with_label_values(&[<$TRequest>::ROUTE])
                    .start_timer();

                let result = $handler(RequestContext {
                    server_state: &$server_state,
                    request,
                    public_key,
                })
                .await;

                timer.observe_duration();

                if let Err(ServerError::InternalError(ref e)) = result {
                    error!("Internal error! Request: {}, Error: {}", request_string, e);
                }
                wrap_err::<$TRequest>(result)
            }
            Err(e) => Err(e),
        })
    }};
}

pub async fn route(
    server_state: &ServerState,
    hyper_request: hyper::Request<Body>,
) -> Result<Response<Body>, hyper::http::Error> {
    match (hyper_request.method(), hyper_request.uri().path()) {
        route_case!(FileMetadataUpsertsRequest) => route_handler!(
            FileMetadataUpsertsRequest,
            file_service::upsert_file_metadata,
            hyper_request,
            server_state
        ),
        route_case!(ChangeDocumentContentRequest) => route_handler!(
            ChangeDocumentContentRequest,
            file_service::change_document_content,
            hyper_request,
            server_state
        ),
        route_case!(GetDocumentRequest) => route_handler!(
            GetDocumentRequest,
            file_service::get_document,
            hyper_request,
            server_state
        ),
        route_case!(GetPublicKeyRequest) => route_handler!(
            GetPublicKeyRequest,
            account_service::get_public_key,
            hyper_request,
            server_state
        ),
        route_case!(GetUsageRequest) => route_handler!(
            GetUsageRequest,
            account_service::get_usage,
            hyper_request,
            server_state
        ),
        route_case!(GetUpdatesRequest) => route_handler!(
            GetUpdatesRequest,
            file_service::get_updates,
            hyper_request,
            server_state
        ),
        route_case!(NewAccountRequest) => route_handler!(
            NewAccountRequest,
            account_service::new_account,
            hyper_request,
            server_state
        ),
        route_case!(GetBuildInfoRequest) => {
            let timer = HTTP_REQUEST_DURATION_HISTOGRAM
                .with_label_values(&[GetBuildInfoRequest::ROUTE])
                .start_timer();
            let result =
                pack::<GetBuildInfoRequest>(wrap_err::<GetBuildInfoRequest>(get_build_info()));
            timer.observe_duration();

            result
        }
        route_case!(MetricsRequest) => metrics(),
        _ => {
            warn!(
                "Request matched no endpoints: {} {}",
                hyper_request.method(),
                hyper_request.uri().path()
            );
            hyper::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(hyper::Body::empty())
        }
    }
}

fn wrap_err<TRequest: Request>(
    result: Result<TRequest::Response, ServerError<TRequest::Error>>,
) -> Result<TRequest::Response, ErrorWrapper<TRequest::Error>> {
    match result {
        Ok(response) => Ok(response),
        Err(ServerError::ClientError(e)) => Err(ErrorWrapper::Endpoint(e)),
        Err(ServerError::InternalError(_)) => Err(ErrorWrapper::InternalError),
    }
}

pub async fn unpack<TRequest: Request + Serialize + DeserializeOwned>(
    server_state: &ServerState,
    hyper_request: hyper::Request<Body>,
) -> Result<(TRequest, PublicKey), ErrorWrapper<TRequest::Error>> {
    let request_bytes = match from_request(hyper_request).await {
        Ok(o) => o,
        Err(e) => {
            warn!("Error getting request bytes: {:?}", e);
            return Err(ErrorWrapper::<TRequest::Error>::BadRequest);
        }
    };
    let request: RequestWrapper<TRequest> = match deserialize_request(request_bytes.clone()) {
        Ok(o) => o,
        Err(e) => {
            warn!(
                "Error deserializing request: {} {:?}",
                String::from_utf8_lossy(&request_bytes),
                e
            );
            return Err(ErrorWrapper::<TRequest::Error>::BadRequest);
        }
    };

    verify_client_version(&request).map_err(|_| {
        warn!("Client connected with unsupported client version");
        ErrorWrapper::<TRequest::Error>::ClientUpdateRequired
    })?;

    match verify_auth(server_state, &request) {
        Ok(()) => {}
        Err(ECVerifyError::SignatureExpired(_)) | Err(ECVerifyError::SignatureInTheFuture(_)) => {
            return Err(ErrorWrapper::<TRequest::Error>::ExpiredAuth);
        }
        Err(_) => {
            return Err(ErrorWrapper::<TRequest::Error>::InvalidAuth);
        }
    }

    Ok((
        request.signed_request.timestamped_value.value,
        request.signed_request.public_key,
    ))
}

pub fn pack<TRequest>(
    result: Result<TRequest::Response, ErrorWrapper<TRequest::Error>>,
) -> Result<hyper::Response<Body>, hyper::http::Error>
where
    TRequest: Request,
    TRequest::Response: Serialize,
    TRequest::Error: Serialize,
{
    let response_bytes = match serialize_response::<TRequest>(result) {
        Ok(o) => o,
        Err(e) => {
            warn!("Error serializing response: {:?}", e);
            return empty_response();
        }
    };

    to_response(response_bytes)
}

pub async fn from_request(request: hyper::Request<Body>) -> Result<Bytes, hyper::Error> {
    body::to_bytes(request.into_body()).await
}

pub fn deserialize_request<TRequest: Request + DeserializeOwned>(
    request: Bytes,
) -> Result<RequestWrapper<TRequest>, serde_json::error::Error> {
    serde_json::from_slice(&request)
}

pub fn serialize_response<TRequest>(
    response: Result<TRequest::Response, ErrorWrapper<TRequest::Error>>,
) -> Result<Bytes, serde_json::error::Error>
where
    TRequest: Request,
    TRequest::Response: Serialize,
    TRequest::Error: Serialize,
{
    Ok(Bytes::from(serde_json::to_vec(&response)?))
}

pub fn to_response(response: Bytes) -> Result<hyper::Response<Body>, hyper::http::Error> {
    hyper::Response::builder()
        .status(StatusCode::OK)
        .body(response.into())
}

pub fn empty_response() -> Result<hyper::Response<Body>, hyper::http::Error> {
    hyper::Response::builder()
        .status(StatusCode::OK)
        .body(hyper::Body::empty())
}
