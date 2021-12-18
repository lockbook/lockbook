extern crate chrono;
extern crate log;
extern crate tokio;

use lockbook_models::api::{
    NewAccountError, NewAccountRequest, NewAccountResponse, Request, RequestWrapper,
};
use lockbook_server_lib::account_service::new_account;
use lockbook_server_lib::config::Config;
use lockbook_server_lib::*;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::future::Future;
use std::io;
use std::sync::Arc;
use warp::http::Method;
use warp::hyper::body::Bytes;
use warp::{reject, Filter, Rejection};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env_vars();
    pretty_env_logger::init();

    let index_db_client = file_index_repo::connect(&config.index_db)
        .await
        .expect("Failed to connect to index_db");

    let files_db_client = file_content_client::create_client(&config.files_db)
        .expect("Failed to create files_db client");

    let server_state = Arc::new(ServerState {
        config: config.clone(),
        index_db_client,
        files_db_client,
    });

    let route = core_request!(NewAccountRequest, new_account, server_state);

    warp::serve(route).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

#[derive(Debug)]
struct MethodError;
impl reject::Reject for MethodError {}

fn method(name: Method) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::method()
        .and(warp::any().map(move || name.clone()))
        .and_then(|request: Method, intention: Method| async move {
            if request == intention {
                Ok(())
            } else {
                Err(reject::custom(MethodError))
            }
        })
        .untuple_one()
}
