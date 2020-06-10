extern crate base64;
extern crate hyper;
extern crate lockbook_core;
extern crate tokio;

#[macro_use]
extern crate log;

pub mod config;
pub mod files_db;
pub mod index_db;
pub mod services;

use crate::config::config;
use hyper::service::{make_service_fn, service_fn};
use hyper::{body, Body, Method, Request, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use services::{
    change_file_content, create_file, delete_file, get_public_key, get_updates, move_file,
    new_account, rename_file,
};
use std::convert::Infallible;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ServerState {
    pub index_db_client: tokio_postgres::Client,
    pub files_db_client: s3::bucket::Bucket,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::init();
    let config = config();

    info!("Connecting to index_db...");
    let index_db_client = index_db::connect(&config.index_db)
        .await
        .expect("Failed to connect to index_db");
    info!("Connected to index_db");

    info!("Connecting to files_db...");
    let files_db_client = files_db::connect(&config.files_db)
        .await
        .expect("Failed to connect to files_db");
    info!("Connected to files_db");

    let server_state = Arc::new(Mutex::new(ServerState {
        index_db_client: index_db_client,
        files_db_client: files_db_client,
    }));
    let addr = format!("0.0.0.0:{}", config.server.port).parse()?;

    let make_service = make_service_fn(|_| {
        let server_state = server_state.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let server_state = server_state.clone();
                route(server_state, req)
            }))
        }
    });

    info!("Serving on port {}", config.server.port);
    hyper::Server::bind(&addr).serve(make_service).await?;
    Ok(())
}

async fn route(
    server_state: Arc<Mutex<ServerState>>,
    request: Request<Body>,
) -> Result<Response<Body>, hyper::http::Error> {
    let mut s = server_state.lock().await;
    match (request.method(), request.uri().path()) {
        (&Method::PUT, "/change-file-content") => {
            info!("Request matched PUT /change-file-content");
            handle(&mut s, request, change_file_content::handle).await
        }
        (&Method::POST, "/create-file") => {
            info!("Request matched POST /create-file");
            handle(&mut s, request, create_file::handle).await
        }
        (&Method::DELETE, "/delete-file") => {
            info!("Request matched DELETE /delete-file");
            handle(&mut s, request, delete_file::handle).await
        }
        (&Method::GET, "/get-public-key") => {
            info!("Request matched GET /get-public-key");
            handle(&mut s, request, get_public_key::handle).await
        }
        (&Method::GET, "/get-updates") => {
            info!("Request matched GET /get-updates");
            handle(&mut s, request, get_updates::handle).await
        }
        (&Method::PUT, "/move-file") => {
            info!("Request matched PUT /move-file");
            handle(&mut s, request, move_file::handle).await
        }
        (&Method::POST, "/new-account") => {
            info!("Request matched POST /new-account");
            handle(&mut s, request, new_account::handle).await
        }
        (&Method::PUT, "/rename-file") => {
            info!("Request matched PUT /rename-file");
            handle(&mut s, request, rename_file::handle).await
        }
        _ => {
            warn!("Request matched no endpoints");
            hyper::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(hyper::Body::empty())
        }
    }
}

async fn handle<'a, Request, Response, ResponseError, Fut>(
    server_state: &'a mut ServerState,
    request: hyper::Request<Body>,
    endpoint_handle: impl FnOnce(&'a mut ServerState, Request) -> Fut
) -> Result<hyper::Response<Body>, hyper::http::Error>
where
    Fut: Future<Output = Result<Response, ResponseError>>,
    Request: DeserializeOwned,
    Response: Serialize,
    ResponseError: Serialize,
{
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
