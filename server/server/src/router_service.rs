use crate::account_service::*;
use crate::billing::billing_service;
use crate::billing::billing_service::*;
use crate::file_service::*;
use crate::utils::get_build_info;
use crate::{handle_version_header, router_service, verify_auth, ServerError, ServerState};
use lazy_static::lazy_static;
use lockbook_shared::api::*;
use lockbook_shared::api::{ErrorWrapper, Request, RequestWrapper};
use lockbook_shared::SharedError;
use prometheus::{
    register_counter_vec, register_histogram_vec, CounterVec, HistogramVec, TextEncoder,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::*;
use warp::http::{HeaderValue, Method, StatusCode};
use warp::hyper::body::Bytes;
use warp::{reject, Filter, Rejection};

lazy_static! {
    pub static ref HTTP_REQUEST_DURATION_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "lockbook_server_request_duration_seconds",
        "Lockbook server's HTTP request duration in seconds",
        &["request"]
    )
    .unwrap();
    pub static ref CORE_VERSION_COUNTER: CounterVec = register_counter_vec!(
        "lockbook_server_core_version",
        "Core version request attempts",
        &["request"]
    )
    .unwrap();
}

#[macro_export]
macro_rules! core_req {
    ($Req: ty, $handler: path, $state: ident) => {{
        use lockbook_shared::api::{ErrorWrapper, Request};
        use lockbook_shared::file_metadata::Owner;
        use tracing::*;
        use $crate::router_service::{self, deserialize_and_check, method};
        use $crate::{RequestContext, ServerError};

        let cloned_state = $state.clone();

        method(<$Req>::METHOD)
            .and(warp::path(&<$Req>::ROUTE[1..]))
            .and(warp::any().map(move || cloned_state.clone()))
            .and(warp::body::bytes())
            .and(warp::header::optional::<String>("Accept-Version"))
            .then(|state: Arc<ServerState>, request: Bytes, version: Option<String>| {
                let span1 = span!(
                    Level::INFO,
                    "matched_request",
                    method = &<$Req>::METHOD.as_str(),
                    route = &<$Req>::ROUTE,
                );
                async move {
                    let state = state.as_ref();
                    let timer = router_service::HTTP_REQUEST_DURATION_HISTOGRAM
                        .with_label_values(&[<$Req>::ROUTE])
                        .start_timer();

                    let request: RequestWrapper<$Req> =
                        match deserialize_and_check(state, request, version) {
                            Ok(req) => req,
                            Err(err) => {
                                warn!("request failed to parse: {:?}", err);
                                return warp::reply::json::<Result<RequestWrapper<$Req>, _>>(&Err(
                                    err,
                                ));
                            }
                        };

                    debug!("request verified successfully");
                    let req_pk = request.signed_request.public_key;
                    let username = match state.index_db.lock().map(|db| {
                        db.accounts
                            .data()
                            .get(&Owner(req_pk))
                            .map(|account| account.username.clone())
                    }) {
                        Ok(Some(username)) => username,
                        Ok(None) => "~unknown~".to_string(),
                        Err(error) => {
                            error!(?error, "hmdb error");
                            "~error~".to_string()
                        }
                    };
                    let req_pk = base64::encode(req_pk.serialize_compressed());

                    let span2 = span!(
                        Level::INFO,
                        "verified_request_signature",
                        username = username.as_str(),
                        public_key = req_pk.as_str()
                    );
                    let rc: RequestContext<$Req> = RequestContext {
                        server_state: state,
                        request: request.signed_request.timestamped_value.value,
                        public_key: request.signed_request.public_key,
                    };
                    async move {
                        let to_serialize = match $handler(rc).await {
                            Ok(response) => {
                                info!("request processed successfully");
                                Ok(response)
                            }
                            Err(ServerError::ClientError(e)) => {
                                warn!("request rejected due to a client error: {:?}", e);
                                Err(ErrorWrapper::Endpoint(e))
                            }
                            Err(ServerError::InternalError(e)) => {
                                error!("Internal error {}: {}", <$Req>::ROUTE, e);
                                Err(ErrorWrapper::InternalError)
                            }
                        };
                        let response = warp::reply::json(&to_serialize);
                        timer.observe_duration();
                        response
                    }
                    .instrument(span2)
                    .await
                }
                .instrument(span1)
            })
    }};
}

pub fn core_routes(
    server_state: &Arc<ServerState>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = Rejection> + Clone {
    core_req!(NewAccountRequest, new_account, server_state)
        .or(core_req!(ChangeDocRequest, change_doc, server_state))
        .or(core_req!(UpsertRequest, upsert_file_metadata, server_state))
        .or(core_req!(GetDocRequest, get_document, server_state))
        .or(core_req!(GetPublicKeyRequest, get_public_key, server_state))
        .or(core_req!(GetUsernameRequest, get_username, server_state))
        .or(core_req!(GetUsageRequest, get_usage, server_state))
        .or(core_req!(GetFileIdsRequest, get_file_ids, server_state))
        .or(core_req!(GetUpdatesRequest, get_updates, server_state))
        .or(core_req!(UpgradeAccountGooglePlayRequest, upgrade_account_google_play, server_state))
        .or(core_req!(UpgradeAccountStripeRequest, upgrade_account_stripe, server_state))
        .or(core_req!(UpgradeAccountAppStoreRequest, upgrade_account_app_store, server_state))
        .or(core_req!(CancelSubscriptionRequest, cancel_subscription, server_state))
        .or(core_req!(GetSubscriptionInfoRequest, get_subscription_info, server_state))
        .or(core_req!(DeleteAccountRequest, delete_account, server_state))
        .or(core_req!(AdminDisappearAccountRequest, admin_disappear_account, server_state))
        .or(core_req!(AdminDisappearFileRequest, admin_disappear_file, server_state))
        .or(core_req!(AdminListUsersRequest, admin_list_users, server_state))
        .or(core_req!(AdminGetAccountInfoRequest, admin_get_account_info, server_state))
        .or(core_req!(AdminValidateAccountRequest, admin_validate_account, server_state))
        .or(core_req!(AdminValidateServerRequest, admin_validate_server, server_state))
        .or(core_req!(AdminFileInfoRequest, admin_file_info, server_state))
        .or(core_req!(AdminRebuildIndexRequest, admin_rebuild_index, server_state))
        .or(core_req!(AdminSetUserTierRequest, admin_set_user_tier, server_state))
}

pub fn build_info() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::get()
        .and(warp::path(&GetBuildInfoRequest::ROUTE[1..]))
        .map(|| {
            let span = span!(
                Level::INFO,
                "matched_request",
                method = &GetBuildInfoRequest::METHOD.as_str(),
                route = &GetBuildInfoRequest::ROUTE,
            );
            let _enter = span.enter();
            let timer = router_service::HTTP_REQUEST_DURATION_HISTOGRAM
                .with_label_values(&[GetBuildInfoRequest::ROUTE])
                .start_timer();
            let resp = get_build_info();
            info!("request processed successfully");
            let resp = warp::reply::json(&resp);
            timer.observe_duration();
            resp
        })
}

pub fn get_metrics() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    warp::get().and(warp::path("metrics")).map(|| {
        let span = span!(Level::INFO, "matched_request", method = "GET", route = "/metrics",);
        let _enter = span.enter();
        match TextEncoder::new().encode_to_string(prometheus::gather().as_slice()) {
            Ok(metrics) => {
                info!("request processed successfully");
                metrics
            }
            Err(err) => {
                error!("Error preparing response for prometheus: {:?}", err);
                String::new()
            }
        }
    })
}

static STRIPE_WEBHOOK_ROUTE: &str = "stripe-webhooks";

pub fn stripe_webhooks(
    server_state: &Arc<ServerState>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let cloned_state = server_state.clone();

    warp::post()
        .and(warp::path(STRIPE_WEBHOOK_ROUTE))
        .and(warp::any().map(move || cloned_state.clone()))
        .and(warp::body::bytes())
        .and(warp::header::header("Stripe-Signature"))
        .then(|state: Arc<ServerState>, request: Bytes, stripe_sig: HeaderValue| async move {
            let span = span!(
                Level::INFO,
                "matched_request",
                method = "POST",
                route = format!("/{}", STRIPE_WEBHOOK_ROUTE).as_str()
            );
            let _enter = span.enter();
            info!("webhook routed");
            let response = span
                .in_scope(|| billing_service::stripe_webhooks(&state, request, stripe_sig))
                .await;

            match response {
                Ok(_) => warp::reply::with_status("".to_string(), StatusCode::OK),
                Err(e) => {
                    error!("{:?}", e);

                    let status_code = match e {
                        ServerError::ClientError(StripeWebhookError::VerificationError(_))
                        | ServerError::ClientError(StripeWebhookError::InvalidBody(_))
                        | ServerError::ClientError(StripeWebhookError::InvalidHeader(_))
                        | ServerError::ClientError(StripeWebhookError::ParseError(_)) => {
                            StatusCode::BAD_REQUEST
                        }
                        ServerError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
                    };

                    warp::reply::with_status("".to_string(), status_code)
                }
            }
        })
}

static PLAY_WEBHOOK_ROUTE: &str = "google_play_notification_webhook";

pub fn google_play_notification_webhooks(
    server_state: &Arc<ServerState>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let cloned_state = server_state.clone();

    warp::post()
        .and(warp::path(PLAY_WEBHOOK_ROUTE))
        .and(warp::any().map(move || cloned_state.clone()))
        .and(warp::body::bytes())
        .and(warp::query::query::<HashMap<String, String>>())
        .then(|state: Arc<ServerState>, request: Bytes, query_parameters: HashMap<String, String>| async move {
            let span =
                span!(Level::INFO, "matched_request", method = "POST", route = format!("/{}", PLAY_WEBHOOK_ROUTE).as_str());
            let _enter = span.enter();
            info!("webhook routed");
            let response = span
                .in_scope(|| billing_service::google_play_notification_webhooks(&state, request, query_parameters))
                .await;
            match response {
                Ok(_) => warp::reply::with_status("".to_string(), StatusCode::OK),
                Err(e) => {
                    error!("{:?}", e);

                    let status_code = match e {
                        ServerError::ClientError(GooglePlayWebhookError::InvalidToken)
                        | ServerError::ClientError(GooglePlayWebhookError::CannotRetrieveData)
                        | ServerError::ClientError(
                            GooglePlayWebhookError::CannotDecodePubSubData(_),
                        ) => StatusCode::BAD_REQUEST,
                        ServerError::ClientError(GooglePlayWebhookError::CannotRetrieveUserInfo)
                        | ServerError::ClientError(
                            GooglePlayWebhookError::CannotRetrievePublicKey,
                        )
                        | ServerError::ClientError(GooglePlayWebhookError::CannotParseTime)
                        | ServerError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
                    };

                    warp::reply::with_status("".to_string(), status_code)
                }
            }
        })
}

static APP_STORE_WEBHOOK_ROUTE: &str = "app_store_notification_webhook";
pub fn app_store_notification_webhooks(
    server_state: &Arc<ServerState>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let cloned_state = server_state.clone();

    warp::post()
        .and(warp::path(APP_STORE_WEBHOOK_ROUTE))
        .and(warp::any().map(move || cloned_state.clone()))
        .and(warp::body::bytes())
        .then(|state: Arc<ServerState>, body: Bytes| async move {
            let span = span!(
                Level::INFO,
                "matched_request",
                method = "POST",
                route = format!("/{}", APP_STORE_WEBHOOK_ROUTE).as_str()
            );
            let _enter = span.enter();
            info!("webhook routed");
            let response = span
                .in_scope(|| billing_service::app_store_notification_webhook(&state, body))
                .await;

            match response {
                Ok(_) => warp::reply::with_status("".to_string(), StatusCode::OK),
                Err(e) => {
                    error!("{:?}", e);

                    let status_code = match e {
                        ServerError::ClientError(AppStoreNotificationError::InvalidJWS) => {
                            StatusCode::BAD_REQUEST
                        }
                        ServerError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
                    };

                    warp::reply::with_status("".to_string(), status_code)
                }
            }
        })
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
    server_state: &ServerState, request: Bytes, version: Option<String>,
) -> Result<RequestWrapper<Req>, ErrorWrapper<Req::Error>>
where
    Req: Request + DeserializeOwned + Serialize,
{
    handle_version_header::<Req>(&server_state.config, &version)?;

    let request = serde_json::from_slice(request.as_ref()).map_err(|err| {
        warn!("Request parsing failure: {}", err);
        ErrorWrapper::<Req::Error>::BadRequest
    })?;

    verify_auth(server_state, &request).map_err(|err| match err {
        SharedError::SignatureExpired(_) | SharedError::SignatureInTheFuture(_) => {
            warn!("expired auth");
            ErrorWrapper::<Req::Error>::ExpiredAuth
        }
        _ => {
            warn!("invalid auth");
            ErrorWrapper::<Req::Error>::InvalidAuth
        }
    })?;

    Ok(request)
}
