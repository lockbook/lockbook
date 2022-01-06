extern crate log;

use deadpool_redis::redis::pipe;

use deadpool_redis::PoolError;
use std::env;
use std::fmt::Debug;

use libsecp256k1::PublicKey;
use lockbook_crypto::pubkey::ECVerifyError;
use lockbook_crypto::{clock_service, pubkey};
use lockbook_models::api::{ErrorWrapper, Request, RequestWrapper};

use serde::{Deserialize, Serialize};

static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct ServerState {
    pub config: config::Config,
    pub index_db_client: sqlx::PgPool,
    pub index_db2_connection: deadpool_redis::Pool,
    pub files_db_client: s3::bucket::Bucket,
}

pub struct RequestContext<'a, TRequest> {
    pub server_state: &'a ServerState,
    pub request: TRequest,
    pub public_key: PublicKey,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerError<U: Debug> {
    ClientError(U),
    InternalError(String),
}

impl<T: Debug> From<PoolError> for ServerError<T> {
    fn from(err: PoolError) -> Self {
        internal!("Could not get conenction for pool: {:?}", err)
    }
}

#[macro_export]
macro_rules! return_if_error {
    ($tx:expr) => {
        match $tx {
            Ok(success) => success,
            Err(TxError::Abort(val)) => return Err(val),
            Err(TxError::Serialization(t)) => {
                return Err(internal!("Failed to serialize value: {:?}", t))
            }
            Err(TxError::DbError(t)) => return Err(internal!("Redis error: {:?}", t)),
        }
    };
}

#[macro_export]
macro_rules! internal {
    ($($arg:tt)*) => {
        crate::ServerError::InternalError(format!($($arg)*))
    };
}

pub fn verify_client_version<Req: Request>(
    request: &RequestWrapper<Req>,
) -> Result<(), ErrorWrapper<Req::Error>> {
    match &request.client_version as &str {
        "0.1.5" => Ok(()),
        _ => Err(ErrorWrapper::<Req::Error>::ClientUpdateRequired),
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

pub mod account_service;
pub mod config;
pub mod file_content_client;
pub mod file_index_repo;
pub mod file_service;
pub mod keys;
pub mod loggers;
pub mod router_service;
pub mod utils;
