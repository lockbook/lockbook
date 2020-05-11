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
use lockbook_core::model::api::{CreateFileError, CreateFileRequest, CreateFileResponse};
use lockbook_core::model::api::{
    ChangeFileContentError, ChangeFileContentRequest, ChangeFileContentResponse,
};
use lockbook_core::model::api::{DeleteFileError, DeleteFileRequest, DeleteFileResponse};
use lockbook_core::model::api::{GetUpdatesError, GetUpdatesRequest, GetUpdatesResponse};
use lockbook_core::model::api::{MoveFileError, MoveFileRequest, MoveFileResponse};
use lockbook_core::model::api::{NewAccountError, NewAccountRequest, NewAccountResponse};
use lockbook_core::model::api::{RenameFileError, RenameFileRequest, RenameFileResponse};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

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
                async move { Ok::<_, Infallible>(handle(server_state, req)) }
            }))
        }
    });

    Server::bind(&addr).serve(make_service).await?;
    Ok(())
}

fn handle(server_state: Arc<Mutex<ServerState>>, req: Request<Body>) -> Response<Body> {
    match (req.method(), req.uri().path()) {
        (&Method::PUT, "/change-file-content") => endpoint::handle::<
            ChangeFileContentRequest,
            ChangeFileContentResponse,
            ChangeFileContentError,
            services::change_file_content::Service,
        >(server_state, req),
        (&Method::POST, "/create-file") => endpoint::handle::<
            CreateFileRequest,
            CreateFileResponse,
            CreateFileError,
            services::create_file::Service,
        >(server_state, req),
        (&Method::DELETE, "/delete-file") => endpoint::handle::<
            DeleteFileRequest,
            DeleteFileResponse,
            DeleteFileError,
            services::delete_file::Service,
        >(server_state, req),
        (&Method::GET, "/get-updates") => endpoint::handle::<
            GetUpdatesRequest,
            GetUpdatesResponse,
            GetUpdatesError,
            services::get_updates::Service,
        >(server_state, req),
        (&Method::PUT, "/move-file") => endpoint::handle::<
            MoveFileRequest,
            MoveFileResponse,
            MoveFileError,
            services::move_file::Service,
        >(server_state, req),
        (&Method::POST, "/new-account") => endpoint::handle::<
            NewAccountRequest,
            NewAccountResponse,
            NewAccountError,
            services::new_account::Service,
        >(server_state, req),
        (&Method::PUT, "/rename-file") => endpoint::handle::<
            RenameFileRequest,
            RenameFileResponse,
            RenameFileError,
            services::rename_file::Service,
        >(server_state, req),
        _ => {
            let mut response = Response::default();
            *response.status_mut() = StatusCode::NOT_FOUND;
            response
        }
    }
}
