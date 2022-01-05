extern crate log;

use deadpool_redis::redis::{cmd, pipe, Pipeline, RedisError, RedisResult};
use deadpool_redis::Connection;
use std::env;
use std::fmt::Debug;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use libsecp256k1::PublicKey;
use lockbook_crypto::pubkey::ECVerifyError;
use lockbook_crypto::{clock_service, pubkey};
use lockbook_models::api::{ErrorWrapper, Request, RequestWrapper};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

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

#[macro_export]
macro_rules! internal {
    ($base:literal $(, $args:tt )*) => {
        crate::ServerError::InternalError(format!($base $(, $args )*))
    };
}

//
//
// pub async fn tx<F, Fut, Out>(c: Connection, keys: &[String], f: F) -> RedisResult<Out>
// where
//     F: FnMut(Arc<Mutex<Connection>>, &mut Pipeline) -> Fut,
//     Fut: Future<Output = RedisResult<Option<Out>>>,
// {
//     let mut f = f;
//     let con = Arc::new(Mutex::new(c));
//     loop {
//         cmd("WATCH").arg(keys).query_async::<_, ()>(con.lock().await.deref_mut()).await?;
//         if let Some(response) = f(con.clone(), pipe().atomic()).await? {
//             break Ok(response);
//         }
//     }
// }

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
pub mod loggers;
pub mod router_service;
pub mod utils;
