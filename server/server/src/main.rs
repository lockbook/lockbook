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

// TODO this must go
fn error(err: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

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

    // // POST /employees/:rate  {"name":"Sean","rate":2}
    // let promote = warp::get()
    //     .and(warp::path("get-build-info"))
    //     .map(|| warp::reply::json(&get_build_info().unwrap()));

    // // POST /employees/:rate  {"name":"Sean","rate":2}
    // let promote = warp::post()
    //     .and(warp::path("new-account"))
    //     .and(warp::body::bytes())
    //     .and(with_state(server_state))
    //     .then(new_acc)
    //     .map(resp_parser);

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

fn authenticated_core_request_handler<Req>(_request: Req, state: Arc<ServerState>) -> impl Filter
where
    Req: Request,
{
    method(Req::METHOD)
        .and(warp::path(&Req::ROUTE[1..]))
        .and(warp::any().map(move || state.clone()))
        .and(warp::body::bytes())
        .then(|state: Arc<ServerState>, request: Bytes| async move {
            let request: RequestWrapper<NewAccountRequest> =
                serde_json::from_slice(request.as_ref()).unwrap();
            let rc: RequestContext<NewAccountRequest> = RequestContext {
                server_state: state.as_ref(),
                request: request.signed_request.timestamped_value.value,
                public_key: request.signed_request.public_key,
            };

            new_account(rc).await
        })
        .boxed()
}

fn with_state(
    state: Arc<ServerState>,
) -> impl Filter<Extract = (Arc<ServerState>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

async fn new_acc(
    request: Bytes,
    state: Arc<ServerState>,
) -> Result<NewAccountResponse, ServerError<NewAccountError>> {
    let request: RequestWrapper<NewAccountRequest> =
        serde_json::from_slice(request.as_ref()).unwrap();
    let rc: RequestContext<NewAccountRequest> = RequestContext {
        server_state: state.as_ref(),
        request: request.signed_request.timestamped_value.value,
        public_key: request.signed_request.public_key,
    };
    let resp = new_account(rc).await;
    // warp::any().map(move || resp.clone())
    resp
}

fn resp_parser(resp: Result<NewAccountResponse, ServerError<NewAccountError>>) -> impl warp::Reply {
    warp::reply::json(&resp.unwrap())
}
