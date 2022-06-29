extern crate log;

use google_androidpublisher3::AndroidPublisher;
use hmdb::errors::Error;
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
use crate::schema::{transaction, ServerV1};
use crate::ServerError::ClientError;

static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct ServerState {
    pub config: config::Config,
    pub index_db: ServerV1,
    pub stripe_client: stripe::Client,
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

impl<E: Debug> From<Error> for ServerError<E> {
    fn from(err: Error) -> Self {
        internal!("hmdb error: {:?}", err)
    }
}

type Tx<'a> = transaction::ServerV1<'a>;

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
pub mod document_service;
pub mod error_handler;
pub mod file_service;
pub mod loggers;
pub mod metrics;
pub mod router_service;
pub mod schema;
pub mod utils;
