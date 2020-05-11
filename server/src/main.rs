extern crate base64;
extern crate hyper;
extern crate lockbook_core;
extern crate tokio;

pub mod api;
pub mod config;
pub mod files_db;
pub mod index_db;
pub mod services;

use crate::config::config;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

pub struct ServerState {
    // index_db_client operations require mutable access for some reason...
    pub index_db_client: Mutex<postgres::Client>,
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
    let server_state = Arc::new(ServerState {
        index_db_client: Mutex::new(index_db_client),
        files_db_client: files_db_client,
    });
    let addr = "0.0.0.0:3000".parse()?;

    let make_service = make_service_fn(move |_| {
        let server_state = server_state.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let server_state = server_state.clone();
                async move { Ok::<_, Infallible>(handle(server_state, req)) }
            }))
        }
    });

    Server::bind(&addr).serve(make_service).await?;
    Ok(())
}

fn handle(server_state: Arc<ServerState>, req: Request<Body>) -> Response<Body> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/new-account") => api::new_account::handle(server_state, req),
        (&Method::POST, "/create-file") => api::create_file::handle(server_state, req),
        (&Method::PUT, "/change-file-content") => {
            api::change_file_content::handle(server_state, req)
        }
        (&Method::PUT, "/rename-file") => api::rename_file::handle(server_state, req),
        (&Method::PUT, "/move-file") => api::move_file::handle(server_state, req),
        (&Method::DELETE, "/delete-file") => api::delete_file::handle(server_state, req),
        (&Method::GET, "/get-updates") => api::get_updates::handle(server_state, req),
        _ => {
            let mut response = Response::default();
            *response.status_mut() = StatusCode::NOT_FOUND;
            response
        }
    }
}
