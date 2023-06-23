use billing::stripe_client::StripeClient;
use google_androidpublisher3::AndroidPublisher;
use std::env;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use libsecp256k1::PublicKey;
use lockbook_shared::api::{ErrorWrapper, Request, RequestWrapper};
use lockbook_shared::{clock, pubkey, SharedError};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::account_service::GetUsageHelperError;
use crate::billing::billing_service::StripeWebhookError;
use crate::billing::stripe_error::SimplifiedStripeError;
use crate::schema::ServerV4;
use crate::ServerError::ClientError;
pub use stripe;
use tracing::log::warn;

static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct ServerState<S: StripeClient> {
    pub config: config::Config,
    pub index_db: Arc<Mutex<ServerV4>>,
    pub stripe_client: S,
    pub google_play_client: AndroidPublisher,
    pub app_store_client: reqwest::Client,
}

#[derive(Clone)]
pub struct RequestContext<TRequest> {
    pub request: TRequest,
    pub public_key: PublicKey,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerError<U: Debug> {
    ClientError(U),
    InternalError(String),
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
    let v = &version.clone().unwrap_or("0.0.0".to_string());
    let Ok(v) = Version::parse(v) else {
        warn!("version not parsable, request rejected: {v}");
        return Err(ErrorWrapper::BadRequest);
    };
    router_service::CORE_VERSION_COUNTER
        .with_label_values(&[&(v.to_string())])
        .inc();
    if !config.server.min_core_version.matches(&v) {
        return Err(ErrorWrapper::<Req::Error>::ClientUpdateRequired);
    }
    Ok(())
}

pub fn verify_auth<TRequest: Request + Serialize, S: StripeClient>(
    server_state: &ServerState<S>, request: &RequestWrapper<TRequest>,
) -> Result<(), SharedError> {
    pubkey::verify(
        &request.signed_request.public_key,
        &request.signed_request,
        server_state.config.server.max_auth_delay as u64,
        server_state.config.server.max_auth_delay as u64,
        clock::get_time,
    )
}

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
