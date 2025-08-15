use crate::billing::app_store_client::AppStoreClient;
use crate::billing::billing_service::*;
use crate::billing::google_play_client::GooglePlayClient;
use crate::billing::stripe_client::StripeClient;
use crate::config::Config;
use crate::document_service::DocumentService;
use crate::utils::get_build_info;
use crate::{ServerError, ServerState, handle_version_header, router_service, verify_auth};
use lazy_static::lazy_static;
use lb_rs::model::api::{ErrorWrapper, Request, RequestWrapper, *};
use lb_rs::model::errors::{LbErrKind, SignError};
use prometheus::{
    CounterVec, HistogramVec, TextEncoder, register_counter_vec, register_histogram_vec,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::*;
use warp::http::{HeaderValue, Method, StatusCode};
use warp::hyper::body::Bytes;
use warp::{Filter, Rejection, reject};

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
        use lb_rs::model::api::{ErrorWrapper, Request};
        use lb_rs::model::file_metadata::Owner;
        use std::net::SocketAddr;
        use tracing::*;
        use $crate::router_service::{self, deserialize_and_check, method};
        use $crate::{RequestContext, ServerError};

        let cloned_state = $state.clone();

        method(<$Req>::METHOD)
            .and(warp::path(&<$Req>::ROUTE[1..]))
            .and(warp::any().map(move || cloned_state.clone()))
            .and(warp::body::bytes())
            .and(warp::header::optional::<String>("Accept-Version"))
            .and(warp::filters::addr::remote())
            .then(
                |state: Arc<ServerState<S, A, G, D>>,
                 request: Bytes,
                 version: Option<String>,
                 ip: Option<SocketAddr>| {
                    let span1 = span!(
                        Level::INFO,
                        "matched_request",
                        http.method = &<$Req>::METHOD.as_str(),
                        http.url = &<$Req>::ROUTE,
                        http.remote_ip = ip
                            .map(|ip| ip.to_string())
                            .unwrap_or_else(|| String::from("unsupported")),
                        core_version = version
                            .clone()
                            .unwrap_or_else(|| String::from("not-present")),
                    );

                    async move {
                        let state = state.as_ref();
                        let timer = router_service::HTTP_REQUEST_DURATION_HISTOGRAM
                            .with_label_values(&[<$Req>::ROUTE])
                            .start_timer();

                        let request: RequestWrapper<$Req> =
                            match deserialize_and_check(&state.config, request, version) {
                                Ok(req) => req,
                                Err(err) => {
                                    warn!("request failed to parse: {:?}", err);
                                    return warp::reply::with_status(
                                        warp::reply::json::<Result<RequestWrapper<$Req>, _>>(&Err(
                                            err,
                                        )),
                                        warp::http::StatusCode::BAD_REQUEST,
                                    );
                                }
                            };

                        debug!("request verified successfully");
                        let req_pk = request.signed_request.public_key;
                        let username = {
                            let db = state.index_db.lock().await;
                            match db
                                .accounts
                                .get()
                                .get(&Owner(req_pk))
                                .map(|account| account.username.clone())
                            {
                                Some(username) => username,
                                None => "~unknown~".to_string(),
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
                            request: request.signed_request.timestamped_value.value,
                            public_key: request.signed_request.public_key,
                        };

                        async move {
                            let status;
                            let log;
                            let mut level = tracing::Level::INFO;
                            let to_serialize = match $handler(state, rc).await {
                                Ok(response) => {
                                    status = warp::http::StatusCode::OK;
                                    log = "request processed successfully".to_string();
                                    Ok(response)
                                }
                                Err(ServerError::ClientError(e)) => {
                                    status = warp::http::StatusCode::BAD_REQUEST;
                                    level = tracing::Level::WARN;
                                    log =
                                        format!("request rejected due to a client error: {:?}", e);
                                    Err(ErrorWrapper::Endpoint(e))
                                }
                                Err(ServerError::InternalError(e)) => {
                                    status = warp::http::StatusCode::INTERNAL_SERVER_ERROR;
                                    level = tracing::Level::ERROR;
                                    log = format!("Internal error {}: {}", <$Req>::ROUTE, e);
                                    Err(ErrorWrapper::InternalError)
                                }
                            };
                            let response =
                                warp::reply::with_status(warp::reply::json(&to_serialize), status);
                            let latency = timer.stop_and_record();
                            match level {
                                tracing::Level::INFO => {
                                    tracing::info!(
                                        http.latency = latency,
                                        http.status = status.as_u16(),
                                        "{log}"
                                    );
                                }
                                tracing::Level::WARN => {
                                    tracing::warn!(
                                        http.latency = latency,
                                        http.status = status.as_u16(),
                                        "{log}"
                                    );
                                }
                                tracing::Level::ERROR => {
                                    tracing::error!(
                                        http.latency = latency,
                                        http.status = status.as_u16(),
                                        "{log}"
                                    );
                                }
                                _ => {
                                    tracing::debug!(
                                        http.latency = latency,
                                        http.status = status.as_u16(),
                                        "{log}"
                                    );
                                }
                            }

                            response
                        }
                        .instrument(span2)
                        .await
                    }
                    .instrument(span1)
                },
            )
    }};
}

pub fn core_routes<S, A, G, D>(
    server_state: &Arc<ServerState<S, A, G, D>>,
) -> impl Filter<Extract = (impl warp::Reply + use<S, A, G, D>,), Error = Rejection>
+ Clone
+ use<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    core_req!(NewAccountRequest, ServerState::new_account, server_state)
        .or(core_req!(NewAccountRequestV2, ServerState::new_account_v2, server_state))
        .or(core_req!(ChangeDocRequest, ServerState::change_doc, server_state))
        .or(core_req!(ChangeDocRequestV2, ServerState::change_doc_v2, server_state))
        .or(core_req!(UpsertRequest, ServerState::upsert_file_metadata, server_state))
        .or(core_req!(UpsertRequestV2, ServerState::upsert_file_metadata_v2, server_state))
        .or(core_req!(GetDocRequest, ServerState::get_document, server_state))
        .or(core_req!(GetPublicKeyRequest, ServerState::get_public_key, server_state))
        .or(core_req!(GetUsernameRequest, ServerState::get_username, server_state))
        .or(core_req!(GetUsageRequest, ServerState::get_usage, server_state))
        .or(core_req!(GetFileIdsRequest, ServerState::get_file_ids, server_state))
        .or(core_req!(GetUpdatesRequest, ServerState::get_updates, server_state))
        .or(core_req!(
            UpgradeAccountGooglePlayRequest,
            ServerState::upgrade_account_google_play,
            server_state
        ))
        .or(core_req!(
            UpgradeAccountStripeRequest,
            ServerState::upgrade_account_stripe,
            server_state
        ))
        .or(core_req!(
            UpgradeAccountAppStoreRequest,
            ServerState::upgrade_account_app_store,
            server_state
        ))
        .or(core_req!(CancelSubscriptionRequest, ServerState::cancel_subscription, server_state))
        .or(core_req!(GetSubscriptionInfoRequest, ServerState::get_subscription_info, server_state))
        .or(core_req!(DeleteAccountRequest, ServerState::delete_account, server_state))
        .or(core_req!(
            AdminDisappearAccountRequest,
            ServerState::admin_disappear_account,
            server_state
        ))
        .or(core_req!(AdminDisappearFileRequest, ServerState::admin_disappear_file, server_state))
        .or(core_req!(AdminListUsersRequest, ServerState::admin_list_users, server_state))
        .or(core_req!(
            AdminGetAccountInfoRequest,
            ServerState::admin_get_account_info,
            server_state
        ))
        .or(core_req!(
            AdminValidateAccountRequest,
            ServerState::admin_validate_account,
            server_state
        ))
        .or(core_req!(AdminValidateServerRequest, ServerState::admin_validate_server, server_state))
        .or(core_req!(AdminFileInfoRequest, ServerState::admin_file_info, server_state))
        .or(core_req!(AdminRebuildIndexRequest, ServerState::admin_rebuild_index, server_state))
        .or(core_req!(AdminSetUserTierRequest, ServerState::admin_set_user_tier, server_state))
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

pub fn stripe_webhooks<S, A, G, D>(
    server_state: &Arc<ServerState<S, A, G, D>>,
) -> impl Filter<Extract = (impl warp::Reply + use<S, A, G, D>,), Error = warp::Rejection>
+ Clone
+ use<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    let cloned_state = server_state.clone();

    warp::post()
        .and(warp::path(STRIPE_WEBHOOK_ROUTE))
        .and(warp::any().map(move || cloned_state.clone()))
        .and(warp::body::bytes())
        .and(warp::header::header("Stripe-Signature"))
        .then(
            |state: Arc<ServerState<S, A, G, D>>, request: Bytes, stripe_sig: HeaderValue| async move {
                let span = span!(
                    Level::INFO,
                    "matched_request",
                    method = "POST",
                    route = format!("/{STRIPE_WEBHOOK_ROUTE}").as_str()
                );
                let _enter = span.enter();
                info!("webhook routed");
                let response = span
                    .in_scope(|| ServerState::stripe_webhooks(&state, request, stripe_sig))
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
            },
        )
}

static PLAY_WEBHOOK_ROUTE: &str = "google_play_notification_webhook";

pub fn google_play_notification_webhooks<S, A, G, D>(
    server_state: &Arc<ServerState<S, A, G, D>>,
) -> impl Filter<Extract = (impl warp::Reply + use<S, A, G, D>,), Error = warp::Rejection>
+ Clone
+ use<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    let cloned_state = server_state.clone();

    warp::post()
        .and(warp::path(PLAY_WEBHOOK_ROUTE))
        .and(warp::any().map(move || cloned_state.clone()))
        .and(warp::body::bytes())
        .and(warp::query::query::<HashMap<String, String>>())
        .then(
            |state: Arc<ServerState<S, A, G, D>>,
             request: Bytes,
             query_parameters: HashMap<String, String>| async move {
                let span = span!(
                    Level::INFO,
                    "matched_request",
                    method = "POST",
                    route = format!("/{PLAY_WEBHOOK_ROUTE}").as_str()
                );
                let _enter = span.enter();
                info!("webhook routed");
                let response = span
                    .in_scope(|| state.google_play_notification_webhooks(request, query_parameters))
                    .await;
                match response {
                    Ok(_) => warp::reply::with_status("".to_string(), StatusCode::OK),
                    Err(e) => {
                        error!("{:?}", e);

                        let status_code = match e {
                            ServerError::ClientError(GooglePlayWebhookError::InvalidToken)
                            | ServerError::ClientError(
                                GooglePlayWebhookError::CannotRetrieveData,
                            )
                            | ServerError::ClientError(
                                GooglePlayWebhookError::CannotDecodePubSubData(_),
                            ) => StatusCode::BAD_REQUEST,
                            ServerError::ClientError(
                                GooglePlayWebhookError::CannotRetrieveUserInfo,
                            )
                            | ServerError::ClientError(
                                GooglePlayWebhookError::CannotRetrievePublicKey,
                            )
                            | ServerError::ClientError(GooglePlayWebhookError::CannotParseTime)
                            | ServerError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
                        };

                        warp::reply::with_status("".to_string(), status_code)
                    }
                }
            },
        )
}

static APP_STORE_WEBHOOK_ROUTE: &str = "app_store_notification_webhook";
pub fn app_store_notification_webhooks<S, A, G, D>(
    server_state: &Arc<ServerState<S, A, G, D>>,
) -> impl Filter<Extract = (impl warp::Reply + use<S, A, G, D>,), Error = warp::Rejection>
+ Clone
+ use<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    let cloned_state = server_state.clone();

    warp::post()
        .and(warp::path(APP_STORE_WEBHOOK_ROUTE))
        .and(warp::any().map(move || cloned_state.clone()))
        .and(warp::body::bytes())
        .then(|state: Arc<ServerState<S, A, G, D>>, body: Bytes| async move {
            let span = span!(
                Level::INFO,
                "matched_request",
                method = "POST",
                route = format!("/{APP_STORE_WEBHOOK_ROUTE}").as_str()
            );
            let _enter = span.enter();
            info!("webhook routed");
            let response = span
                .in_scope(|| state.app_store_notification_webhook(body))
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
            if request == intention { Ok(()) } else { Err(reject::not_found()) }
        })
        .untuple_one()
}

pub fn deserialize_and_check<Req>(
    config: &Config, request: Bytes, version: Option<String>,
) -> Result<RequestWrapper<Req>, ErrorWrapper<Req::Error>>
where
    Req: Request + DeserializeOwned + Serialize,
{
    handle_version_header::<Req>(config, &version)?;

    let request = serde_json::from_slice(request.as_ref()).map_err(|err| {
        warn!("Request parsing failure: {}", err);
        ErrorWrapper::<Req::Error>::BadRequest
    })?;

    verify_auth(config, &request).map_err(|err| match err.kind {
        LbErrKind::Sign(SignError::SignatureExpired(_))
        | LbErrKind::Sign(SignError::SignatureInTheFuture(_)) => {
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
