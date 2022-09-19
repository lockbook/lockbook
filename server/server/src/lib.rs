use google_androidpublisher3::AndroidPublisher;
use hmdb::errors::Error;
use std::env;
use std::fmt::Debug;

use libsecp256k1::PublicKey;
use lockbook_shared::api::{ErrorWrapper, Request, RequestWrapper};
use lockbook_shared::{clock, pubkey, SharedError};
use serde::{Deserialize, Serialize};

use crate::account_service::GetUsageHelperError;
use crate::billing::billing_service::StripeWebhookError;
use crate::billing::stripe_client::SimplifiedStripeError;
use crate::billing::stripe_model::{StripeDeclineCodeCatcher, StripeKnownDeclineCode};
use crate::schema::v2::{transaction, Server};
use crate::ServerError::ClientError;

static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct ServerState {
    pub config: config::Config,
    pub index_db: Server,
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

type Tx<'a> = transaction::Server<'a>;

#[macro_export]
macro_rules! internal {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        tracing::error!("{}", msg);
        $crate::ServerError::InternalError(msg)
    }};
}

pub fn verify_client_version<Req: Request>(
    request: &RequestWrapper<Req>,
) -> Result<(), ErrorWrapper<Req::Error>> {
    match &request.client_version as &str {
        "0.5.2" => Ok(()),
        "0.5.3" => Ok(()),
        "0.5.4" => Ok(()),
        _ => Err(ErrorWrapper::<Req::Error>::ClientUpdateRequired),
    }
}

pub fn verify_auth<TRequest: Request + Serialize>(
    server_state: &ServerState, request: &RequestWrapper<TRequest>,
) -> Result<(), SharedError> {
    pubkey::verify(
        &request.signed_request.public_key,
        &request.signed_request,
        server_state.config.server.max_auth_delay as u64,
        server_state.config.server.max_auth_delay as u64,
        clock::get_time,
    )
}

pub const FREE_TIER_USAGE_SIZE: u64 = 1000000;
pub const PREMIUM_TIER_USAGE_SIZE: u64 = 30000000000;

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
