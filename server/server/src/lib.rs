pub mod account_service;
pub mod config;
pub mod file_content_client;
pub mod file_index_repo;
pub mod file_service;
pub mod loggers;
pub mod router_service;
pub mod utils;
pub mod https_service;

extern crate log;
use std::{env, fs, io};

use hyper::body::Bytes;
use hyper::{Body, Response};
use libsecp256k1::PublicKey;
use lockbook_crypto::pubkey::ECVerifyError;
use lockbook_crypto::{clock_service, pubkey};
use lockbook_models::api::{GetBuildInfoError, GetBuildInfoResponse, Request, RequestWrapper};

use prometheus::TextEncoder;
use serde::Serialize;
use shadow_rs::shadow;

shadow!(build_info);

static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

// TODO this must go
fn error(err: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

pub struct ServerState {
    pub config: config::Config,
    pub index_db_client: sqlx::PgPool,
    pub files_db_client: s3::bucket::Bucket,
}

pub struct RequestContext<'a, TRequest> {
    pub server_state: &'a ServerState,
    pub request: TRequest,
    pub public_key: PublicKey,
}

pub enum ServerError<U> {
    ClientError(U),
    InternalError(String),
}

// Load public certificate from file.
pub fn load_certs(filename: &str) -> io::Result<Vec<rustls::Certificate>> {
    // Open certificate file.
    let certfile = fs::File::open(filename)
        .map_err(|e| error(format!("failed to open {}: {}", filename, e)))?;
    let mut reader = io::BufReader::new(certfile);

    // Load and return certificate.
    let certs = rustls_pemfile::certs(&mut reader)
        .map_err(|_| error("failed to load certificate".into()))?;
    Ok(certs.into_iter().map(rustls::Certificate).collect())
}

// Load private key from file.
pub fn load_private_key(filename: &str) -> io::Result<rustls::PrivateKey> {
    // Open keyfile.
    let keyfile = fs::File::open(filename)
        .map_err(|e| error(format!("failed to open {}: {}", filename, e)))?;
    let mut reader = io::BufReader::new(keyfile);

    // Load and return a single private key.
    let keys = rustls_pemfile::rsa_private_keys(&mut reader)
        .map_err(|_| error("failed to load private key".into()))?;
    if keys.len() != 1 {
        return Err(error("expected a single private key".into()));
    }

    Ok(rustls::PrivateKey(keys[0].clone()))
}

pub fn get_build_info() -> Result<GetBuildInfoResponse, ServerError<GetBuildInfoError>> {
    Ok(GetBuildInfoResponse {
        build_version: env!("CARGO_PKG_VERSION"),
        git_commit_hash: build_info::COMMIT_HASH,
    })
}

pub fn metrics() -> Result<Response<Body>, hyper::http::Error> {
    match TextEncoder::new().encode_to_string(prometheus::gather().as_slice()) {
        Ok(metrics) => router_service::to_response(Bytes::from(metrics)),
        Err(e) => {
            router_service::to_response(Bytes::from(format!("Could not encode metrics: {:?}", e)))
        }
    }
}

pub fn verify_client_version<TRequest: Request>(
    request: &RequestWrapper<TRequest>,
) -> Result<(), ()> {
    match &request.client_version as &str {
        "0.1.5" => Ok(()),
        _ => Err(()),
    }
}

pub fn verify_auth<TRequest: Request + Serialize>(
    server_state: &ServerState,
    request: &RequestWrapper<TRequest>,
) -> Result<(), ECVerifyError> {
    pubkey::verify(
        &request.signed_request.public_key,
        &request.signed_request,
        server_state.config.server.max_auth_delay as u64,
        server_state.config.server.max_auth_delay as u64,
        clock_service::get_time,
    )
}
