use billing::app_store_client::AppStoreClient;
use billing::google_play_client::GooglePlayClient;
use billing::stripe_client::StripeClient;
use defense::IpData;
use document_service::DocumentService;
use lb_rs::model::clock;
use lb_rs::model::errors::LbResult;
use schema::ServerDb;
use std::collections::VecDeque;
use std::env;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

use lb_rs::model::api::{ErrorWrapper, Request, RequestWrapper};
use lb_rs::model::pubkey;
use libsecp256k1::PublicKey;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::ServerError::ClientError;
use crate::account_service::GetUsageHelperError;
use crate::billing::billing_service::StripeWebhookError;
use crate::billing::stripe_error::SimplifiedStripeError;
pub use stripe;
use tracing::log::warn;

static CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub config: config::Config,
    pub index_db: Arc<Mutex<ServerDb>>,
    pub stripe_client: S,
    pub google_play_client: G,
    pub app_store_client: A,
    pub document_service: D,
    pub recent_new_account_ips: Arc<Mutex<VecDeque<IpData>>>,
}

#[derive(Clone)]
pub struct RequestContext<TRequest> {
    pub request: TRequest,
    pub public_key: PublicKey,
    pub ip: Option<SocketAddr>,
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

pub fn verify_auth<TRequest>(
    config: &config::Config, request: &RequestWrapper<TRequest>,
) -> LbResult<()>
where
    TRequest: Request + Serialize,
{
    pubkey::verify(
        &request.signed_request.public_key,
        &request.signed_request,
        config.server.max_auth_delay as u64,
        config.server.max_auth_delay as u64,
        clock::get_time,
    )
}

pub mod account_service;
pub mod billing;
pub mod config;
pub mod defense;
pub mod document_service;
pub mod error_handler;
pub mod file_service;
pub mod loggers;
pub mod metrics;
pub mod router_service;
pub mod schema;
pub mod utils;
