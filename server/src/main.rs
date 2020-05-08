#![feature(proc_macro_hygiene, decl_macro)]

extern crate base64;
extern crate hyper;
extern crate lockbook_core;
extern crate tokio;

pub mod api;
pub mod config;
pub mod files_db;
pub mod index_db;
pub mod services;

use crate::config::{config, ServerState};
use std::sync::Mutex;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};

async fn handle(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/new-account") => Ok(Response::new(req.into_body())),
        (&Method::POST, "/create-file") => Ok(Response::new(req.into_body())),
        (&Method::PUT, "/change-file-content") => Ok(Response::new(req.into_body())),
        (&Method::PUT, "/rename-file") => Ok(Response::new(req.into_body())),
        (&Method::PUT, "/move-file") => Ok(Response::new(req.into_body())),
        (&Method::DELETE, "/delete-file") => Ok(Response::new(req.into_body())),
        (&Method::GET, "/get-updates") => Ok(Response::new(req.into_body())),
        _ => {
            let mut response = Response::default();
            *response.status_mut() = StatusCode::NOT_FOUND;
            Ok(response)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = config();
    let index_db_client = match index_db::connect(&config.index_db_config) {
        Ok(client) => client,
        Err(err) => panic!("{:?}", err),
    };
    let files_db_client = match files_db::connect(&config.files_db_config) {
        Ok(x) => x,
        Err(err) => panic!("{:?}", err),
    };
    let server_state = ServerState {
        index_db_client: Mutex::new(index_db_client),
        files_db_client: Mutex::new(files_db_client),
    };

    let addr = ([127, 0, 0, 1], 3000).into();
    let service = make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(|req: Request<Body>| {handle(req)})) });
    let server = Server::bind(&addr).serve(service);
    println!("Listening on http://{}", addr);
    server.await?;
    Ok(())
}
