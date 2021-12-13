extern crate chrono;
extern crate log;
extern crate tokio;

use futures_util::future::TryFutureExt;
use hyper::server::accept;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Server};
use lockbook_server_lib::config::Config;
use lockbook_server_lib::*;
use std::convert::Infallible;
use std::sync::Arc;
use std::{io, sync};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

// TODO this must go
fn error(err: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env_vars();

    loggers::init(&config);

    let index_db_client = file_index_repo::connect(&config.index_db)
        .await
        .expect("Failed to connect to index_db");

    let files_db_client = file_content_client::create_client(&config.files_db)
        .expect("Failed to create files_db client");

    let server_state = Arc::new(ServerState {
        config: config.clone(),
        index_db_client,
        files_db_client,
    });

    

    Ok(())
}

