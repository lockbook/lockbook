extern crate chrono;
extern crate hyper;
extern crate tokio;

#[macro_use]
extern crate log;

use hyper::body::Bytes;
use hyper::service::{make_service_fn, service_fn};
use hyper::{body, Body, Response, StatusCode};
use libsecp256k1::PublicKey;
use lockbook_crypto::pubkey::ECVerifyError;
use lockbook_crypto::{clock_service, pubkey};
use lockbook_models::api::*;
use lockbook_server_lib::config::Config;
use lockbook_server_lib::*;
use prometheus::{register_histogram_vec, TextEncoder};
use serde::de::DeserializeOwned;
use serde::Serialize;
use shadow_rs::shadow;
use std::convert::Infallible;
use std::path::Path;
use std::sync::Arc;
use tokio::runtime::Handle;

static LOG_FILE: &str = "lockbook_server.log";
static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

shadow!(build_info);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let handle = Handle::current();
    let config = Config::from_env_vars();

    loggers::init(
        Path::new(&config.server.log_path),
        LOG_FILE,
        true,
        &config.server.pd_api_key,
        handle,
        CARGO_PKG_VERSION,
    )
    .expect(format!("Logger failed to initialize at {}", &config.server.log_path).as_str())
    .level(log::LevelFilter::Info)
    .level_for("lockbook_server", log::LevelFilter::Debug)
    .apply()
    .expect("Failed setting logger!");
    info!("Server starting with build: {}", CARGO_PKG_VERSION);

    debug!("Connecting to index_db...");
    let index_db_client = file_index_repo::connect(&config.index_db)
        .await
        .expect("Failed to connect to index_db");
    debug!("Connected to index_db");

    debug!("Connecting to files_db...");
    let files_db_client = file_content_client::create_client(&config.files_db)
        .expect("Failed to connect to files_db");
    debug!("Connected to files_db");

    let port = config.server.port;
    let server_state = Arc::new(ServerState {
        config,
        index_db_client,
        files_db_client,
        prom_his: register_histogram_vec!(
            "http_request_duration_seconds",
            "The HTTP requests duration in seconds.",
            &["request"]
        )
        .unwrap(),
    });
    let addr = format!("0.0.0.0:{}", port).parse()?;

    // https://www.fpcomplete.com/blog/ownership-puzzle-rust-async-hyper/
    let make_service = make_service_fn(move |_| {
        let server_state = Arc::clone(&server_state);
        async {
            Ok::<_, Infallible>(service_fn(move |req: hyper::Request<Body>| {
                let server_state = Arc::clone(&server_state);
                async move { route(&server_state, req).await }
            }))
        }
    });

    info!("Serving on port {}", port);
    hyper::Server::bind(&addr)
        .http1_keepalive(false)
        .http2_keep_alive_interval(None)
        .serve(make_service)
        .await?;

    Ok(())
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
                let timer = $server_state
                    .prom_his
                    .with_label_values(&[<$TRequest>::ROUTE])
                    .start_timer();

                let result = $handler(RequestContext {
                    server_state: &$server_state,
                    request,
                    public_key,
                })
                .await;

                timer.observe_duration();

                if let Err(Err(ref e)) = result {
                    error!("Internal error! Request: {}, Error: {}", request_string, e);
                }
                wrap_err::<$TRequest>(result)
            }
            Err(e) => Err(e),
        })
    }};
}

async fn route(
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
            let timer = server_state
                .prom_his
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

fn get_build_info() -> Result<GetBuildInfoResponse, Result<GetBuildInfoError, String>> {
    Ok(GetBuildInfoResponse {
        build_version: env!("CARGO_PKG_VERSION"),
        git_commit_hash: build_info::COMMIT_HASH,
    })
}

fn metrics() -> Result<Response<Body>, hyper::http::Error> {
    match TextEncoder::new().encode_to_string(prometheus::gather().as_slice()) {
        Ok(metrics) => to_response(Bytes::from(metrics)),
        Err(e) => to_response(Bytes::from(format!("Could not encode metrics: {:?}", e))),
    }
}

fn wrap_err<TRequest: Request>(
    result: Result<TRequest::Response, Result<TRequest::Error, String>>,
) -> Result<TRequest::Response, ErrorWrapper<TRequest::Error>> {
    match result {
        Ok(response) => Ok(response),
        Err(Ok(e)) => Err(ErrorWrapper::Endpoint(e)),
        Err(Err(_)) => Err(ErrorWrapper::InternalError),
    }
}

async fn unpack<TRequest: Request + Serialize + DeserializeOwned>(
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

fn pack<TRequest>(
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

async fn from_request(request: hyper::Request<Body>) -> Result<Bytes, hyper::Error> {
    body::to_bytes(request.into_body()).await
}

fn deserialize_request<TRequest: Request + DeserializeOwned>(
    request: Bytes,
) -> Result<RequestWrapper<TRequest>, serde_json::error::Error> {
    serde_json::from_slice(&request)
}

fn verify_client_version<TRequest: Request>(request: &RequestWrapper<TRequest>) -> Result<(), ()> {
    match &request.client_version as &str {
        "0.1.5" => Ok(()),
        _ => Err(()),
    }
}

fn verify_auth<TRequest: Request + Serialize>(
    server_state: &ServerState,
    request: &RequestWrapper<TRequest>,
) -> Result<(), ECVerifyError> {
    pubkey::verify(
        &request.signed_request.public_key,
        &request.signed_request,
        server_state.config.server.max_auth_delay as u64,
        server_state.config.server.max_auth_delay as u64,
        clock_service::get_time,
    )
}

fn serialize_response<TRequest>(
    response: Result<TRequest::Response, ErrorWrapper<TRequest::Error>>,
) -> Result<Bytes, serde_json::error::Error>
where
    TRequest: Request,
    TRequest::Response: Serialize,
    TRequest::Error: Serialize,
{
    Ok(Bytes::from(serde_json::to_vec(&response)?))
}

fn to_response(response: Bytes) -> Result<hyper::Response<Body>, hyper::http::Error> {
    hyper::Response::builder()
        .status(StatusCode::OK)
        .body(response.into())
}

fn empty_response() -> Result<hyper::Response<Body>, hyper::http::Error> {
    hyper::Response::builder()
        .status(StatusCode::OK)
        .body(hyper::Body::empty())
}
