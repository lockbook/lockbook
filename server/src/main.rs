extern crate base64;
extern crate hyper;
extern crate lockbook_core;
extern crate tokio;

pub mod config;
pub mod files_db;
pub mod index_db;
pub mod services;

use crate::config::config;
use hyper::service::{make_service_fn, service_fn};
use hyper::{body, Body, Method, Request, Response, StatusCode};
use lockbook_core::service::logging_service::Logger;
use serde::de::DeserializeOwned;
use serde::Serialize;
use services::{
    change_file_content, create_file, delete_file, get_updates, move_file, new_account, rename_file,
};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::Mutex;

type Log = lockbook_core::service::logging_service::ConditionalStdOut;

pub struct ServerState {
    pub index_db_client: tokio_postgres::Client,
    pub files_db_client: s3::bucket::Bucket,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = config();
    let index_db_client =
        index_db::connect(&config.index_db_config).expect("Failed to connect to index_db");
    let files_db_client =
        files_db::connect(&config.files_db_config).expect("Failed to connect to files_db");
    let server_state = Arc::new(Mutex::new(ServerState {
        index_db_client: index_db_client,
        files_db_client: files_db_client,
    }));
    let addr = "0.0.0.0:3000".parse()?;

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
            Log::info(String::from("Request matched PUT /change-file-content"));
            serialize(
                deserialize(request)
                    .await
                    .map(|r| change_file_content::handle(&mut s, r)),
            )
        }
        (&Method::POST, "/create-file") => {
            Log::info(String::from("Request matched POST /create-file"));
            serialize(
                deserialize(request)
                    .await
                    .map(|r| create_file::handle(&mut s, r)),
            )
        }
        (&Method::DELETE, "/delete-file") => {
            Log::info(String::from("Request matched DELETE /delete-file"));
            serialize(
                deserialize(request)
                    .await
                    .map(|r| delete_file::handle(&mut s, r)),
            )
        }
        (&Method::GET, "/get-updates") => {
            Log::info(String::from("Request matched GET /get-updates"));
            serialize(
                deserialize(request)
                    .await
                    .map(|r| get_updates::handle(&mut s, r)),
            )
        }
        (&Method::PUT, "/move-file") => {
            Log::info(String::from("Request matched PUT /move-file"));
            serialize(
                deserialize(request)
                    .await
                    .map(|r| move_file::handle(&mut s, r)),
            )
        }
        (&Method::POST, "/new-account") => {
            Log::info(String::from("Request matched POST /new-account"));
            serialize(
                deserialize(request)
                    .await
                    .map(|r| new_account::handle(&mut s, r)),
            )
        }
        (&Method::PUT, "/rename-file") => {
            Log::info(String::from("Request matched PUT /rename-file"));
            serialize(
                deserialize(request)
                    .await
                    .map(|r| rename_file::handle(&mut s, r)),
            )
        }
        _ => {
            Log::warn(String::from("Request matched no endpoints"));
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
            Log::debug(String::from(format!("Response: {:?}", body)));
            hyper::Response::builder()
                .status(StatusCode::OK)
                .body(body.into())
        }
        Err(err) => {
            Log::warn(String::from(format!("Response: {:?}", err)));
            hyper::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(hyper::Body::empty())
        }
    }
}
