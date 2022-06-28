extern crate log;

use google_androidpublisher3::AndroidPublisher;
use std::env;
use std::fmt::Debug;

use libsecp256k1::PublicKey;
use lockbook_crypto::pubkey::ECVerifyError;
use lockbook_crypto::{clock_service, pubkey};
use lockbook_models::api::{ErrorWrapper, Request, RequestWrapper};
use serde::{Deserialize, Serialize};

use crate::account_service::GetUsageHelperError;
use crate::billing::billing_service::StripeWebhookError;
use crate::billing::stripe_client::SimplifiedStripeError;
use crate::billing::stripe_model::{StripeDeclineCodeCatcher, StripeKnownDeclineCode};
use crate::content::file_content_client;
use crate::ServerError::ClientError;

static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct ServerState {
    pub config: config::Config,
    pub index_db_pool: deadpool_redis::Pool,
    pub stripe_client: stripe::Client,
    pub files_db_client: s3::bucket::Bucket,
    pub google_play_client: AndroidPublisher,
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

pub fn verify_client_version<Req: Request>(
    request: &RequestWrapper<Req>,
) -> Result<(), ErrorWrapper<Req::Error>> {
    match &request.client_version as &str {
        "0.4.3" => Ok(()),
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
pub const PREMIUM_TIER_USAGE_SIZE: u64 = 50000000000;

pub mod account_service;
pub mod billing;
pub mod config;
pub mod content;
pub mod error_handler;
pub mod feature_flags;
pub mod file_service;
pub mod keys;
pub mod loggers;
pub mod metrics;
pub mod router_service;
pub mod utils;
