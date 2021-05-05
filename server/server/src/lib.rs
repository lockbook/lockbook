pub mod account_service;
pub mod config;
pub mod file_content_client;
pub mod file_index_repo;
pub mod file_service;
pub mod loggers;
pub mod utils;

use rsa::RSAPublicKey;

#[macro_use]
extern crate log;

pub struct ServerState {
    pub config: config::Config,
    pub index_db_client: tokio_postgres::Client,
    pub files_db_client: s3::bucket::Bucket,
}

pub struct RequestContext<'a, TRequest> {
    pub server_state: &'a mut ServerState,
    pub request: TRequest,
    pub public_key: RSAPublicKey,
}
