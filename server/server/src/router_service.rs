use std::sync::Arc;
use crate::{verify_auth, verify_client_version, ServerState};
use lazy_static::lazy_static;
use lockbook_crypto::pubkey::ECVerifyError;
use lockbook_models::api::{ErrorWrapper, Request, RequestWrapper};
use log::warn;
use prometheus::{register_histogram_vec, HistogramVec};
use serde::de::DeserializeOwned;
use serde::Serialize;
use warp::http::Method;
use warp::hyper::body::Bytes;
use warp::{reject, Filter, Rejection};
use lockbook_models::api::*;
use crate::account_service::new_account;
use crate::file_service::*;


lazy_static! {
    pub static ref HTTP_REQUEST_DURATION_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "lockbook_server_request_duration_seconds",
        "The lockbook server's HTTP requests duration in seconds.",
        &["request"]
    )
    .unwrap();
}

#[macro_export]
macro_rules! core_request {
    ($Req: ty, $handler: path, $state: ident) => {{
        use crate::{RequestContext, ServerError};
        use lockbook_models::api::{ErrorWrapper, Request};
        use log::error;
        use crate::router_service::{deserialize_and_check, method};
        use crate::router_service;

        let cloned_state = Arc::clone(&$state);

        method(<$Req>::METHOD)
            .and(warp::path(&<$Req>::ROUTE[1..]))
            .and(warp::any().map(move || Arc::clone(&cloned_state)))
            .and(warp::body::bytes())
            .then(|state: Arc<ServerState>, request: Bytes| async move {
                let state = state.as_ref();

                let timer = router_service::HTTP_REQUEST_DURATION_HISTOGRAM
                    .with_label_values(&[<$Req>::ROUTE])
                    .start_timer();

                let request: RequestWrapper<$Req> = match deserialize_and_check(state, request) {
                    Ok(req) => req,
                    Err(err) => {
                        return warp::reply::json::<Result<RequestWrapper<$Req>, _>>(&Err(err));
                    }
                };

                let rc: RequestContext<$Req> = RequestContext {
                    server_state: state,
                    request: request.signed_request.timestamped_value.value,
                    public_key: request.signed_request.public_key,
                };

                let to_serialize = match $handler(rc).await {
                    Ok(response) => Ok(response),
                    Err(ServerError::ClientError(e)) => Err(ErrorWrapper::Endpoint(e)),
                    Err(ServerError::InternalError(e)) => {
                        error!("Internal error {}: {}", <$Req>::ROUTE, e);
                        Err(ErrorWrapper::InternalError)
                    }
                };
                let response = warp::reply::json(&to_serialize);
                timer.observe_duration();
                response
            })
    }};
}

pub fn core_routes(server_state: &Arc<ServerState>) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    core_request!(NewAccountRequest, new_account, server_state)
        .or(core_request!(
            FileMetadataUpsertsRequest,
            upsert_file_metadata,
            server_state
        ))
        .or(core_request!(
            ChangeDocumentContentRequest,
            change_document_content,
            server_state
        ))
}

pub fn method(name: Method) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::method()
        .and(warp::any().map(move || name.clone()))
        .and_then(|request: Method, intention: Method| async move {
            if request == intention {
                Ok(())
            } else {
                Err(reject::not_found())
            }
        })
        .untuple_one()
}

pub fn deserialize_and_check<Req>(
    server_state: &ServerState,
    request: Bytes,
) -> Result<RequestWrapper<Req>, ErrorWrapper<Req::Error>>
where
    Req: Request,
    Req: DeserializeOwned,
    Req: Serialize,
{
    let request = serde_json::from_slice(request.as_ref()).map_err(|err| {
        warn!("Request parsing failure: {}", err);
        ErrorWrapper::<Req::Error>::BadRequest
    })?;

    verify_client_version(&request).map_err(|_| {
        warn!("Client connected with unsupported client version");
        ErrorWrapper::<Req::Error>::ClientUpdateRequired
    })?;

    verify_auth(server_state, &request).map_err(|err| match err {
        ECVerifyError::SignatureExpired(_) | ECVerifyError::SignatureInTheFuture(_) => {
            ErrorWrapper::<Req::Error>::ClientUpdateRequired
        }
        _ => ErrorWrapper::<Req::Error>::InvalidAuth,
    })?;

    Ok(request)
}
