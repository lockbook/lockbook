extern crate log;

use deadpool_redis::redis::RedisError;
use deadpool_redis::PoolError;

use std::env;
use std::fmt::Debug;

use crate::billing::stripe_client::StripeClientError;
use crate::ServerError::{ClientError, InternalError};
use libsecp256k1::PublicKey;
use lockbook_crypto::pubkey::ECVerifyError;
use lockbook_crypto::{clock_service, pubkey};
use lockbook_models::api::{ErrorWrapper, Request, RequestWrapper, SwitchAccountTierError};
use redis_utils::converters::JsonGetError;
use serde::{Deserialize, Serialize};

static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct ServerState {
    pub config: config::Config,
    pub index_db_pool: deadpool_redis::Pool,
    pub stripe_client: reqwest::Client,
    pub files_db_client: s3::bucket::Bucket,
}

#[derive(Clone)]
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

// TODO these should probably have backtraces on them
impl<T: Debug> From<PoolError> for ServerError<T> {
    fn from(err: PoolError) -> Self {
        internal!("Could not get conenction for pool: {:?}", err)
    }
}

// TODO these should probably have backtraces on them
impl<T: Debug> From<RedisError> for ServerError<T> {
    fn from(err: RedisError) -> Self {
        internal!("Redis Error: {:?}", err)
    }
}

// TODO these should probably have backtraces on them
impl<T: Debug> From<JsonGetError> for ServerError<T> {
    fn from(err: JsonGetError) -> Self {
        internal!("Redis Error: {:?}", err)
    }
}

#[macro_export]
macro_rules! return_if_error {
    ($tx:expr) => {
        match $tx {
            Ok(success) => success,
            Err(redis_utils::TxError::Abort(val)) => return Err(val),
            Err(redis_utils::TxError::Serialization(t)) => {
                return Err(internal!("Failed to serialize value: {:?}", t))
            }
            Err(redis_utils::TxError::DbError(t)) => return Err(internal!("Redis error: {:?}", t)),
        }
    };
}

#[macro_export]
macro_rules! internal {
    ($($arg:tt)*) => {
        crate::ServerError::InternalError(format!($($arg)*))
    };
}

impl From<StripeClientError> for ServerError<SwitchAccountTierError> {
    fn from(e: StripeClientError) -> Self {
        match e {
            StripeClientError::CardDeclined(decline_type) => {
                ClientError(SwitchAccountTierError::CardDeclined(decline_type))
            }
            StripeClientError::InvalidCreditCard(field) => {
                ClientError(SwitchAccountTierError::InvalidCreditCard(field))
            }
            StripeClientError::Other(msg) => InternalError(msg),
        }
    }
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

pub const FREE_TIER_USAGE_SIZE: u64 = 1000000;
pub const MONTHLY_TIER_USAGE_SIZE: u64 = 50000000000;

pub mod account_service;
pub mod billing;
pub mod config;
pub mod file_content_client;
pub mod file_service;
pub mod keys;
pub mod loggers;
pub mod router_service;
pub mod utils;
