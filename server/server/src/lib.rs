extern crate log;

use deadpool_redis::redis::RedisError;
use deadpool_redis::PoolError;

use std::env;
use std::fmt::Debug;

use crate::billing::stripe_client::SimplifiedStripeError;
use crate::ServerError::ClientError;
use libsecp256k1::PublicKey;
use lockbook_crypto::pubkey::ECVerifyError;
use lockbook_crypto::{clock_service, pubkey};
use lockbook_models::api::{ErrorWrapper, Request, RequestWrapper, SwitchAccountTierError};
use redis_utils::converters::{JsonGetError, JsonSetError};

use crate::billing::billing_service::StripeWebhookError;
use crate::billing::stripe_model::{StripeKnownErrorDeclineCode, StripeMaybeContainer};
use crate::content::file_content_client;
use serde::{Deserialize, Serialize};
use stripe::WebhookError;

static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct ServerState {
    pub config: config::Config,
    pub index_db_pool: deadpool_redis::Pool,
    pub stripe_client: stripe::Client,
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

impl<T: Debug> From<PoolError> for ServerError<T> {
    fn from(err: PoolError) -> Self {
        internal!("Could not get conenction for pool: {:?}", err)
    }
}

impl<T: Debug> From<RedisError> for ServerError<T> {
    fn from(err: RedisError) -> Self {
        internal!("Redis Error: {:?}", err)
    }
}

impl<T: Debug> From<JsonGetError> for ServerError<T> {
    fn from(err: JsonGetError) -> Self {
        internal!("Redis Error: {:?}", err)
    }
}

impl<T: Debug> From<JsonSetError> for ServerError<T> {
    fn from(err: JsonSetError) -> Self {
        internal!("Redis Error: {:?}", err)
    }
}

impl<T: Debug> From<file_content_client::Error> for ServerError<T> {
    fn from(err: file_content_client::Error) -> Self {
        internal!("S3 Error: {:?}", err)
    }
}

impl<T: Debug> From<Box<bincode::ErrorKind>> for ServerError<T> {
    fn from(err: Box<bincode::ErrorKind>) -> Self {
        internal!("bincode error: {:?}", err)
    }
}

impl<T: Debug> From<stripe::ParseIdError> for ServerError<T> {
    fn from(err: stripe::ParseIdError) -> Self {
        internal!("stripe parse error: {:?}", err)
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
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        log::error!("{}", msg);
        crate::ServerError::InternalError(msg)
    }};
}

impl From<SimplifiedStripeError> for ServerError<SwitchAccountTierError> {
    fn from(e: SimplifiedStripeError) -> Self {
        match e {
            SimplifiedStripeError::CardDeclined(decline_type) => {
                ClientError(SwitchAccountTierError::CardDeclined(decline_type))
            }
            SimplifiedStripeError::InvalidCreditCard(field) => {
                ClientError(SwitchAccountTierError::InvalidCreditCard(field))
            }
            SimplifiedStripeError::Other(msg) => internal!("{}", msg),
        }
    }
}

impl From<stripe::WebhookError> for ServerError<StripeWebhookError> {
    fn from(e: WebhookError) -> Self {
        match e {
            WebhookError::BadKey => {
                internal!("Cannot verify stripe request because server is using a bad signing key.")
            }
            WebhookError::BadHeader(bad_header_err) => {
                ClientError(StripeWebhookError::InvalidHeader(format!("{:?}", bad_header_err)))
            }
            WebhookError::BadSignature => {
                ClientError(StripeWebhookError::InvalidHeader("Bad signature.".to_string()))
            }
            WebhookError::BadTimestamp(bad_timestamp_err) => {
                ClientError(StripeWebhookError::InvalidHeader(format!(
                    "Timestamp for webhook is too old: {}",
                    bad_timestamp_err
                )))
            }
            WebhookError::BadParse(bad_parse_err) => {
                ClientError(StripeWebhookError::ParseError(format!("{:?}", bad_parse_err)))
            }
        }
    }
}

pub fn verify_client_version<Req: Request>(
    request: &RequestWrapper<Req>,
) -> Result<(), ErrorWrapper<Req::Error>> {
    match &request.client_version as &str {
        "0.1.6" => Ok(()),
        _ => Err(ErrorWrapper::<Req::Error>::ClientUpdateRequired),
    }
}

pub fn verify_auth<TRequest: Request + Serialize>(
    server_state: &ServerState, request: &RequestWrapper<TRequest>,
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
pub mod content;
pub mod feature_flags;
pub mod file_service;
pub mod keys;
pub mod loggers;
pub mod metrics;
pub mod router_service;
pub mod utils;
