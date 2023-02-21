use google_androidpublisher3::AndroidPublisher;
use hmdb::errors::Error;
use std::env;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use libsecp256k1::PublicKey;
use lockbook_shared::api::{ErrorWrapper, Request, RequestWrapper};
use lockbook_shared::{clock, pubkey, SharedError};
use serde::{Deserialize, Serialize};

use crate::account_service::GetUsageHelperError;
use crate::billing::billing_service::StripeWebhookError;
use crate::billing::stripe_client::SimplifiedStripeError;
use crate::billing::stripe_model::{StripeDeclineCodeCatcher, StripeKnownDeclineCode};
use crate::schema::ServerV4;
use crate::ServerError::ClientError;
pub use stripe;

static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct ServerState {
    pub config: config::Config,
    pub index_db: Arc<Mutex<ServerV4>>,
    pub stripe_client: stripe::Client,
    pub google_play_client: AndroidPublisher,
    pub app_store_client: reqwest::Client,
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

#[macro_export]
macro_rules! internal {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        tracing::error!("{}", msg);
        $crate::ServerError::InternalError(msg)
    }};
}

pub fn handle_version_header<Req: Request>(
    config: &config::Config, version: &Option<String>,
) -> Result<(), ErrorWrapper<Req::Error>> {
    let incompatible_versions = &config.server.deprecated_core_versions;
    let v = &version.clone().unwrap_or_default();
    if version.is_none() || incompatible_versions.contains(v) {
        return Err(ErrorWrapper::<Req::Error>::ClientUpdateRequired);
    }
    router_service::CORE_VERSION_COUNTER
        .with_label_values(&[v])
        .inc();
    Ok(())
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
