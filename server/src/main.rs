extern crate base64;
extern crate chrono;
extern crate hyper;
extern crate tokio;

#[macro_use]
extern crate log;

pub mod account_service;
pub mod config;
pub mod file_content_client;
pub mod file_index_repo;
pub mod file_service;
mod loggers;
pub mod usage_service;
pub mod utils;

use crate::config::config;
use hyper::body::Bytes;
use hyper::service::{make_service_fn, service_fn};
use hyper::{body, Body, Response, StatusCode};
use lockbook_crypto::clock_service::ClockImpl;
use lockbook_crypto::crypto_service::{PubKeyCryptoService, RSAImpl, RSAVerifyError};
use lockbook_models::api::*;
use rsa::RSAPublicKey;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::convert::Infallible;
use std::path::Path;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::Mutex;

static LOG_FILE: &str = "lockbook_server.log";

pub struct ServerState {
    pub config: config::Config,
    pub index_db_client: tokio_postgres::Client,
    pub files_db_client: s3::bucket::Bucket,
}

pub struct RequestContext<'a, TRequest> {
    server_state: &'a mut ServerState,
    request: TRequest,
    public_key: RSAPublicKey,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let handle = Handle::current();
    let config = config();

    loggers::init(
        Path::new(&config.server.log_path),
        LOG_FILE.to_string(),
        true,
        &config.server.pd_api_key,
        handle,
    )
    .expect(format!("Logger failed to initialize at {}", &config.server.log_path).as_str())
    .level(log::LevelFilter::Info)
    .level_for("lockbook_server", log::LevelFilter::Debug)
    .apply()
    .expect("Failed setting logger!");

    info!("Connecting to index_db...");
    let index_db_client = file_index_repo::connect(&config.index_db)
        .await
        .expect("Failed to connect to index_db");
    info!("Connected to index_db");

    info!("Connecting to files_db...");
    let files_db_client = file_content_client::connect(&config.files_db)
        .await
        .expect("Failed to connect to files_db");
    info!("Connected to files_db");

    let port = config.server.port;
    let server_state = Arc::new(Mutex::new(ServerState {
        config: config,
        index_db_client: index_db_client,
        files_db_client: files_db_client,
    }));
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let make_service = make_service_fn(|_| {
        let server_state = server_state.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req: hyper::Request<Body>| {
                let server_state = server_state.clone();
                route(server_state, req)
            }))
        }
    });

    info!("Serving on port {}", port);
    hyper::Server::bind(&addr)
        .http1_keepalive(false)
        .http2_keep_alive_interval(None)
        .serve(make_service);
    // .await?;

    error!("An error has occurred!");

    Ok(())
}

macro_rules! route_case {
    ($TRequest:ty) => {
        (&<$TRequest>::METHOD, <$TRequest>::ROUTE)
    };
}

macro_rules! route_handler {
    ($TRequest:ty, $handler:path, $hyper_request:ident, $s: ident) => {{
        info!(
            "Request matched {}{}",
            <$TRequest>::METHOD,
            <$TRequest>::ROUTE
        );

        let result = match unpack(&$s, $hyper_request).await {
            Ok((request, public_key)) => {
                debug!("Request: {:?}", request);
                wrap_err::<$TRequest>(
                    $handler(&mut RequestContext {
                        server_state: &mut $s,
                        request,
                        public_key,
                    })
                    .await,
                )
            }
            Err(e) => Err(e),
        };
        debug!("Response: {:?}", result);
        pack::<$TRequest>(result)
    }};
}

async fn route(
    server_state: Arc<Mutex<ServerState>>,
    hyper_request: hyper::Request<Body>,
) -> Result<Response<Body>, hyper::http::Error> {
    let mut s = server_state.lock().await;
    reconnect(&mut s).await;
    match (hyper_request.method(), hyper_request.uri().path()) {
        route_case!(ChangeDocumentContentRequest) => route_handler!(
            ChangeDocumentContentRequest,
            file_service::change_document_content,
            hyper_request,
            s
        ),
        route_case!(CreateDocumentRequest) => route_handler!(
            CreateDocumentRequest,
            file_service::create_document,
            hyper_request,
            s
        ),
        route_case!(DeleteDocumentRequest) => route_handler!(
            DeleteDocumentRequest,
            file_service::delete_document,
            hyper_request,
            s
        ),
        route_case!(MoveDocumentRequest) => route_handler!(
            MoveDocumentRequest,
            file_service::move_document,
            hyper_request,
            s
        ),
        route_case!(RenameDocumentRequest) => route_handler!(
            RenameDocumentRequest,
            file_service::rename_document,
            hyper_request,
            s
        ),
        route_case!(GetDocumentRequest) => route_handler!(
            GetDocumentRequest,
            file_service::get_document,
            hyper_request,
            s
        ),
        route_case!(CreateFolderRequest) => route_handler!(
            CreateFolderRequest,
            file_service::create_folder,
            hyper_request,
            s
        ),
        route_case!(DeleteFolderRequest) => route_handler!(
            DeleteFolderRequest,
            file_service::delete_folder,
            hyper_request,
            s
        ),
        route_case!(MoveFolderRequest) => route_handler!(
            MoveFolderRequest,
            file_service::move_folder,
            hyper_request,
            s
        ),
        route_case!(RenameFolderRequest) => route_handler!(
            RenameFolderRequest,
            file_service::rename_folder,
            hyper_request,
            s
        ),
        route_case!(GetPublicKeyRequest) => route_handler!(
            GetPublicKeyRequest,
            account_service::get_public_key,
            hyper_request,
            s
        ),
        route_case!(GetUpdatesRequest) => route_handler!(
            GetUpdatesRequest,
            file_service::get_updates,
            hyper_request,
            s
        ),
        route_case!(NewAccountRequest) => route_handler!(
            NewAccountRequest,
            account_service::new_account,
            hyper_request,
            s
        ),
        route_case!(GetUsageRequest) => route_handler!(
            GetUsageRequest,
            account_service::get_usage,
            hyper_request,
            s
        ),
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
    result: Result<TRequest::Response, Option<TRequest::Error>>,
) -> Result<TRequest::Response, ErrorWrapper<TRequest::Error>> {
    match result {
        Ok(response) => Ok(response),
        Err(Some(e)) => Err(ErrorWrapper::Endpoint(e)),
        Err(None) => Err(ErrorWrapper::InternalError),
    }
}

async fn reconnect(server_state: &mut ServerState) {
    if server_state.index_db_client.is_closed() {
        match file_index_repo::connect(&server_state.config.index_db).await {
            Err(e) => {
                error!("Failed to reconnect to postgres: {:?}", e);
            }
            Ok(client) => {
                server_state.index_db_client = client;
                info!("Reconnected to index_db");
            }
        }
    }
}

async fn unpack<TRequest: Request + Serialize + DeserializeOwned>(
    server_state: &ServerState,
    hyper_request: hyper::Request<Body>,
) -> Result<(TRequest, RSAPublicKey), ErrorWrapper<TRequest::Error>> {
    let request_bytes = match from_request(hyper_request).await {
        Ok(o) => o,
        Err(e) => {
            warn!("Error getting request bytes: {:?}", e);
            return Err(ErrorWrapper::<TRequest::Error>::BadRequest);
        }
    };
    let request: RequestWrapper<TRequest> = match deserialize_request(request_bytes) {
        Ok(o) => o,
        Err(e) => {
            warn!("Error deserializing request: {:?}", e);
            return Err(ErrorWrapper::<TRequest::Error>::BadRequest);
        }
    };

    verify_client_version(&request).map_err(|_| {
        warn!("Client connected with unsupported client version");
        ErrorWrapper::<TRequest::Error>::ClientUpdateRequired
    })?;

    match verify_auth(server_state, &request) {
        Ok(()) => {}
        Err(RSAVerifyError::SignatureExpired(_)) | Err(RSAVerifyError::SignatureInTheFuture(_)) => {
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
        "0.0.0" => Err(()),
        "0.1.0" => Ok(()),
        "0.1.1" => Ok(()),
        "0.1.2" => Ok(()),
        "0.1.3" => Ok(()),
        _ => Err(()),
    }
}

fn verify_auth<TRequest: Request + Serialize>(
    server_state: &ServerState,
    request: &RequestWrapper<TRequest>,
) -> Result<(), RSAVerifyError> {
    RSAImpl::<ClockImpl>::verify(
        &request.signed_request.public_key,
        &request.signed_request,
        server_state.config.server.max_auth_delay as u64,
        server_state.config.server.max_auth_delay as u64,
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
