pub mod account_service;
pub mod config;
pub mod file_content_client;
pub mod file_index_repo;
pub mod file_service;
pub mod loggers;
pub mod payment_service;
pub mod utils;

extern crate log;

use libsecp256k1::PublicKey;

pub struct ServerState {
    pub config: config::Config,
    pub stripe_client: stripe::Client,
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
