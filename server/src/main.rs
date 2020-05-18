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
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ServerState {
    pub index_db_client: tokio_postgres::Client,
    pub files_db_client: s3::bucket::Bucket,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting server on port 8000");

    let config = config();
    let index_db_client = index_db::connect(&config.index_db_config)
        .await
        .expect("Failed to connect to index_db");
    let files_db_client =
        files_db::connect(&config.files_db_config).expect("Failed to connect to files_db");
    let server_state = Arc::new(Mutex::new(ServerState {
        index_db_client: index_db_client,
        files_db_client: files_db_client,
    }));
    let addr = "0.0.0.0:8000".parse()?;

    let make_service = make_service_fn(|_| {
        let server_state = server_state.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let server_state = server_state.clone();
                handle(server_state, req)
            }))
        }
    });

    hyper::Server::bind(&addr).serve(make_service).await?;
    Ok(())
}

async fn handle(
    server_state: Arc<Mutex<ServerState>>,
    request: Request<Body>,
) -> Result<Response<Body>, hyper::http::Error> {
    let mut s = server_state.lock().await;
    match (request.method(), request.uri().path()) {
        (&Method::PUT, "/change-file-content") => {
            info!("Request matched PUT /change-file-content");
            serialize(match deserialize(request).await {
                Ok(req) => Ok(change_file_content::handle(&mut s, req).await),
                Err(err) => Err(err),
            })
        }
        (&Method::POST, "/create-file") => {
            info!("Request matched POST /create-file");
            serialize(match deserialize(request).await {
                Ok(req) => Ok(create_file::handle(&mut s, req).await),
                Err(err) => Err(err),
            })
        }
        (&Method::DELETE, "/delete-file") => {
            info!("Request matched DELETE /delete-file");
            serialize(match deserialize(request).await {
                Ok(req) => Ok(delete_file::handle(&mut s, req).await),
                Err(err) => Err(err),
            })
        }
        (&Method::GET, "/get-public-key") => {
            info!("Request matched GET /get-public-key");
            serialize(match deserialize(request).await {
                Ok(req) => Ok(get_public_key::handle(&mut s, req).await),
                Err(err) => Err(err),
            })
        }
        (&Method::GET, "/get-updates") => {
            info!("Request matched GET /get-updates");
            serialize(match deserialize(request).await {
                Ok(req) => Ok(get_updates::handle(&mut s, req).await),
                Err(err) => Err(err),
            })
        }
        (&Method::PUT, "/move-file") => {
            info!("Request matched PUT /move-file");
            serialize(match deserialize(request).await {
                Ok(req) => Ok(move_file::handle(&mut s, req).await),
                Err(err) => Err(err),
            })
        }
        (&Method::POST, "/new-account") => {
            info!("Request matched POST /new-account");
            serialize(match deserialize(request).await {
                Ok(req) => Ok(new_account::handle(&mut s, req).await),
                Err(err) => Err(err),
            })
        }
        (&Method::PUT, "/rename-file") => {
            info!("Request matched PUT /rename-file");
            serialize(match deserialize(request).await {
                Ok(req) => Ok(rename_file::handle(&mut s, req).await),
                Err(err) => Err(err),
            })
        }
        _ => {
            warn!("Request matched no endpoints");
            hyper::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(hyper::Body::empty())
        }
    }
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
        .map_err(|e| Error::HyperBodyToBytes(e))?;
    let body_string =
        String::from_utf8(body_bytes.to_vec()).map_err(|e| Error::HyperBodyBytesToString(e))?;
    let request = serde_json::from_str(&body_string).map_err(|e| Error::JsonDeserialize(e))?;
    Ok(request)
}

fn serialize<'a, Response: Serialize, ResponseError: Serialize>(
    response: Result<Result<Response, ResponseError>, Error>,
) -> Result<hyper::Response<Body>, hyper::http::Error> {
    let response_body =
        response.and_then(|r| serde_json::to_string(&r).map_err(|e| Error::JsonSerialize(e)));
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
