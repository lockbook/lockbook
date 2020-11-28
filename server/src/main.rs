extern crate base64;
extern crate chrono;
extern crate hyper;
extern crate lockbook_models;
extern crate tokio;

#[macro_use]
extern crate log;

pub mod account_service;
pub mod config;
pub mod file_content_client;
pub mod file_index_repo;
pub mod file_service;
pub mod usage_service;
pub mod utils;

use crate::config::config;
use hyper::service::{make_service_fn, service_fn};
use hyper::{body, Body, Method, Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::convert::Infallible;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

static LOG_FILE: &str = "lockbook_server.log";

pub struct ServerState {
    pub config: config::Config,
    pub index_db_client: tokio_postgres::Client,
    pub files_db_client: s3::bucket::Bucket,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = config();

    let log_path = Path::new(&config.server.log_path);
    let _ = fs::create_dir_all(log_path)?;
    let log_file = fern::log_file(log_path.join(LOG_FILE))?;

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{timestamp}] [{target:<40}] [{level:<5}]: {message}\x1B[0m",
                timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                target = record.target(),
                level = record.level(),
                message = message.clone(),
            ))
        })
        .chain(std::io::stdout())
        .chain(log_file)
        .level(log::LevelFilter::Info)
        .level_for("lockbook_server", log::LevelFilter::Debug)
        .apply()
        .expect(format!("Logger failed to initialize at {}", &config.server.log_path).as_str());

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
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let server_state = server_state.clone();
                route(server_state, req)
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

async fn route(
    server_state: Arc<Mutex<ServerState>>,
    request: Request<Body>,
) -> Result<Response<Body>, hyper::http::Error> {
    let mut s = server_state.lock().await;
    match (request.method(), request.uri().path()) {
        (&Method::PUT, "/change-document-content") => {
            info!("Request matched PUT /change-document-content");
            handle(&mut s, request, file_service::change_document_content).await
        }
        (&Method::POST, "/create-document") => {
            info!("Request matched POST /create-document");
            handle(&mut s, request, file_service::create_document).await
        }
        (&Method::DELETE, "/delete-document") => {
            info!("Request matched DELETE /delete-document");
            handle(&mut s, request, file_service::delete_document).await
        }
        (&Method::PUT, "/move-document") => {
            info!("Request matched PUT /move-document");
            handle(&mut s, request, file_service::move_document).await
        }
        (&Method::PUT, "/rename-document") => {
            info!("Request matched PUT /rename-document");
            handle(&mut s, request, file_service::rename_document).await
        }
        (&Method::GET, "/get-document") => {
            info!("Request matched GET /get-document");
            handle(&mut s, request, file_service::get_document).await
        }
        (&Method::POST, "/create-folder") => {
            info!("Request matched POST /create-folder");
            handle(&mut s, request, file_service::create_folder).await
        }
        (&Method::DELETE, "/delete-folder") => {
            info!("Request matched DELETE /delete-folder");
            handle(&mut s, request, file_service::delete_folder).await
        }
        (&Method::PUT, "/move-folder") => {
            info!("Request matched PUT /move-folder");
            handle(&mut s, request, file_service::move_folder).await
        }
        (&Method::PUT, "/rename-folder") => {
            info!("Request matched PUT /rename-folder");
            handle(&mut s, request, file_service::rename_folder).await
        }
        (&Method::GET, "/get-public-key") => {
            info!("Request matched GET /get-public-key");
            handle(&mut s, request, account_service::get_public_key).await
        }
        (&Method::GET, "/get-updates") => {
            info!("Request matched GET /get-updates");
            handle(&mut s, request, file_service::get_updates).await
        }
        (&Method::POST, "/new-account") => {
            info!("Request matched POST /new-account");
            handle(&mut s, request, account_service::new_account).await
        }
        (&Method::GET, "/get-usage") => {
            info!("Request matched GET /get-usage");
            handle(&mut s, request, account_service::calculate_usage).await
        }
        _ => {
            warn!(
                "Request matched no endpoints: {} {}",
                request.method(),
                request.uri().path()
            );
            hyper::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(hyper::Body::empty())
        }
    }
}

async fn handle<'a, Request, Response, ResponseError, Fut>(
    server_state: &'a mut ServerState,
    request: hyper::Request<Body>,
    endpoint_handle: impl FnOnce(&'a mut ServerState, Request) -> Fut,
) -> Result<hyper::Response<Body>, hyper::http::Error>
where
    Fut: Future<Output = Result<Response, ResponseError>>,
    Request: DeserializeOwned,
    Response: Serialize,
    ResponseError: Serialize,
{
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
    serialize::<Response, ResponseError>(match deserialize::<Request>(request).await {
        Ok(req) => Ok(endpoint_handle(server_state, req).await),
        Err(err) => Err(err),
    })
}

#[derive(Debug)]
enum Error {
    HyperBodyToBytes(hyper::Error),
    HyperBodyBytesToString(std::string::FromUtf8Error),
    JsonDeserialize(serde_json::error::Error),
    JsonSerialize(serde_json::error::Error),
}

async fn deserialize<Request: DeserializeOwned>(
    request: hyper::Request<Body>,
) -> Result<Request, Error> {
    let body_bytes = body::to_bytes(request.into_body())
        .await
        .map_err(Error::HyperBodyToBytes)?;
    let body_string =
        String::from_utf8(body_bytes.to_vec()).map_err(Error::HyperBodyBytesToString)?;
    let request = serde_json::from_str(&body_string).map_err(Error::JsonDeserialize)?;
    Ok(request)
}

fn serialize<Response: Serialize, ResponseError: Serialize>(
    response: Result<Result<Response, ResponseError>, Error>,
) -> Result<hyper::Response<Body>, hyper::http::Error> {
    let response_body =
        response.and_then(|r| serde_json::to_string(&r).map_err(Error::JsonSerialize));
    match response_body {
        Ok(body) => {
            debug!("Response: {:?}", body);
            hyper::Response::builder()
                .status(StatusCode::OK)
                .body(body.into())
        }
        Err(err) => {
            warn!("Response: {:?}", err);
            hyper::Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(hyper::Body::empty())
        }
    }
}
