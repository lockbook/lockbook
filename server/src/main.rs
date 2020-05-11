extern crate base64;
extern crate hyper;
extern crate lockbook_core;
extern crate tokio;

pub mod config;
pub mod endpoint;
pub mod files_db;
pub mod index_db;
pub mod services;

use crate::config::config;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use lockbook_core::model::api::{CreateFileError, CreateFileRequest, CreateFileResponse};

pub struct ServerState {
    pub index_db_client: postgres::Client,
    pub files_db_client: s3::bucket::Bucket,
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
                async move { Ok::<_, Infallible>(handle(server_state, req).await) }
            }))
        }
    });

    Server::bind(&addr).serve(make_service).await?;
    Ok(())
}

async fn handle(server_state: Arc<Mutex<ServerState>>, req: Request<Body>) -> Response<Body> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/new-account") => Response::default(),
        (&Method::POST, "/create-file") => endpoint::handle::<CreateFileRequest, CreateFileResponse, CreateFileError, services::create_file::Service>(server_state, req).await,
        (&Method::PUT, "/change-file-content") => Response::default(),
        (&Method::PUT, "/rename-file") => Response::default(),
        (&Method::PUT, "/move-file") => Response::default(),
        (&Method::DELETE, "/delete-file") => Response::default(),
        (&Method::GET, "/get-updates") => Response::default(),
        _ => {
            let mut response = Response::default();
            *response.status_mut() = StatusCode::NOT_FOUND;
            response
        }
    }
}
