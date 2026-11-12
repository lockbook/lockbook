use std::sync::{Arc, LazyLock};
use web_time::{Duration, Instant};

#[cfg(not(target_family = "wasm"))]
use bytes::Bytes;
#[cfg(not(target_family = "wasm"))]
use futures::stream;
#[cfg(not(target_family = "wasm"))]
use reqwest::Certificate;
use reqwest::{Body, Client};

use crate::get_code_version;
use crate::io::server::{ApiResult, Server};
use crate::model::account::Account;
use crate::model::api::*;
use crate::model::clock::{Timestamp, get_time};
use crate::model::core_config::ClientType;
use crate::model::errors::LbErr;
use crate::model::pubkey;
use crate::model::wire::{CLIENT_HEADER, OS_HEADER, WIRE_FORMAT_HEADER, WireFormat};

const STREAM_CHUNK_BYTES: usize = 4 * 1024 * 1024;

const STREAM_BODY_THRESHOLD: usize = 1024 * 1024 * 1024;

impl<E> From<ErrorWrapper<E>> for ApiError<E> {
    fn from(err: ErrorWrapper<E>) -> Self {
        match err {
            ErrorWrapper::Endpoint(e) => ApiError::Endpoint(e),
            ErrorWrapper::ClientUpdateRequired => ApiError::ClientUpdateRequired,
            ErrorWrapper::InvalidAuth => ApiError::InvalidAuth,
            ErrorWrapper::ExpiredAuth => ApiError::ExpiredAuth,
            ErrorWrapper::InternalError => ApiError::InternalError,
            ErrorWrapper::BadRequest => ApiError::BadRequest,
        }
    }
}

// #[derive(Debug, PartialEq, Eq)]
#[derive(Debug)]
pub enum ApiError<E> {
    Endpoint(E),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
    Sign(LbErr),
    Serialize(String),
    SendFailed(String),
    ReceiveFailed(String),
    Deserialize(String),
}

#[cfg(not(target_family = "wasm"))]
static SYSTEM_ROOTS: LazyLock<Vec<Certificate>> = LazyLock::new(|| {
    rustls_native_certs::load_native_certs()
        .unwrap_or_default()
        .iter()
        .filter_map(|root| Certificate::from_der(&root.0).ok())
        .collect()
});

fn client() -> Client {
    let builder = Client::builder();
    #[cfg(not(target_family = "wasm"))]
    let builder = SYSTEM_ROOTS
        .iter()
        .cloned()
        .fold(builder, |builder, root| builder.add_root_certificate(root));
    builder.build().unwrap_or_default()
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Network {
    pub client: Arc<LazyLock<Client>>,
    pub get_code_version: fn() -> &'static str,
    pub get_time: fn() -> Timestamp,
    pub client_type: ClientType,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            client: Arc::new(LazyLock::new(client)),
            get_code_version,
            get_time,
            client_type: ClientType::Unknown,
        }
    }
}

impl Network {
    #[instrument(level = "debug", skip(self, account, request), fields(route=T::ROUTE), err(Debug))]
    pub async fn request<T: Request>(
        &self, account: &Account, request: T,
    ) -> Result<T::Response, ApiError<T::Error>> {
        let signed_request =
            pubkey::sign(&account.private_key, &account.public_key(), request, self.get_time)
                .map_err(ApiError::Sign)?;

        let client_version = String::from((self.get_code_version)());

        let wire_format = WireFormat::CLIENT_DEFAULT;
        let serialized_request = wire_format
            .serialize(&RequestWrapper { signed_request, client_version: client_version.clone() })
            .map_err(|err| ApiError::Serialize(err.to_string()))?;

        if serialized_request.len() > 10 * 1024 * 1024 {
            warn!(
                "making network request with {} bytes ({:?})",
                serialized_request.len(),
                wire_format
            );
        }

        let url = &account.api_url;
        let start = Instant::now();
        let body = body_for(serialized_request);
        let sent = self
            .client
            .request(T::METHOD, format!("{}{}", url, T::ROUTE).as_str())
            .body(body)
            .header("Accept-Version", client_version)
            .header(WIRE_FORMAT_HEADER, wire_format.as_str())
            .header(OS_HEADER, client_os())
            .header(CLIENT_HEADER, self.client_type.as_str())
            .send()
            .await
            .map_err(|e| {
                warn!("send failed: {e:?}");
                ApiError::SendFailed(e.to_string())
            })?;
        if start.elapsed() > Duration::from_millis(1000) {
            warn!("network request took {:?}", start.elapsed());
        }

        let serialized_response = sent
            .bytes()
            .await
            .map_err(|err| ApiError::ReceiveFailed(err.to_string()))?;
        let response: Result<T::Response, ErrorWrapper<T::Error>> = wire_format
            .deserialize(&serialized_response)
            .map_err(|err| ApiError::Deserialize(err.to_string()))?;
        response.map_err(ApiError::from)
    }
}

fn client_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "ios") {
        "iOS"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
}

#[cfg(not(target_family = "wasm"))]
fn body_for(serialized_request: Vec<u8>) -> Body {
    if serialized_request.len() < STREAM_BODY_THRESHOLD {
        return Body::from(serialized_request);
    }
    let mut buf = Bytes::from(serialized_request);
    let mut chunks: Vec<Result<Bytes, std::io::Error>> =
        Vec::with_capacity(buf.len().div_ceil(STREAM_CHUNK_BYTES));
    while !buf.is_empty() {
        let n = buf.len().min(STREAM_CHUNK_BYTES);
        chunks.push(Ok(buf.split_to(n)));
    }
    Body::wrap_stream(stream::iter(chunks))
}

#[cfg(target_family = "wasm")]
fn body_for(serialized_request: Vec<u8>) -> Body {
    Body::from(serialized_request)
}

impl Server for Network {
    async fn new_account_v2(
        &self, account: &Account, request: NewAccountRequestV2,
    ) -> ApiResult<NewAccountRequestV2> {
        self.request(account, request).await
    }

    async fn change_doc_v2(
        &self, account: &Account, request: ChangeDocRequestV2,
    ) -> ApiResult<ChangeDocRequestV2> {
        self.request(account, request).await
    }

    async fn upsert_file_metadata_v2(
        &self, account: &Account, request: UpsertRequestV2,
    ) -> ApiResult<UpsertRequestV2> {
        self.request(account, request).await
    }

    async fn get_document(
        &self, account: &Account, request: GetDocRequest,
    ) -> ApiResult<GetDocRequest> {
        self.request(account, request).await
    }

    async fn get_public_key(
        &self, account: &Account, request: GetPublicKeyRequest,
    ) -> ApiResult<GetPublicKeyRequest> {
        self.request(account, request).await
    }

    async fn get_username(
        &self, account: &Account, request: GetUsernameRequest,
    ) -> ApiResult<GetUsernameRequest> {
        self.request(account, request).await
    }

    async fn get_usage(
        &self, account: &Account, request: GetUsageRequest,
    ) -> ApiResult<GetUsageRequest> {
        self.request(account, request).await
    }

    async fn get_file_ids(
        &self, account: &Account, request: GetFileIdsRequest,
    ) -> ApiResult<GetFileIdsRequest> {
        self.request(account, request).await
    }

    async fn get_updates_v2(
        &self, account: &Account, request: GetUpdatesRequestV2,
    ) -> ApiResult<GetUpdatesRequestV2> {
        self.request(account, request).await
    }

    async fn get_build_info(
        &self, account: &Account, request: GetBuildInfoRequest,
    ) -> ApiResult<GetBuildInfoRequest> {
        self.request(account, request).await
    }

    async fn delete_account(
        &self, account: &Account, request: DeleteAccountRequest,
    ) -> ApiResult<DeleteAccountRequest> {
        self.request(account, request).await
    }

    async fn upgrade_account_stripe(
        &self, account: &Account, request: UpgradeAccountStripeRequest,
    ) -> ApiResult<UpgradeAccountStripeRequest> {
        self.request(account, request).await
    }

    async fn upgrade_account_google_play(
        &self, account: &Account, request: UpgradeAccountGooglePlayRequest,
    ) -> ApiResult<UpgradeAccountGooglePlayRequest> {
        self.request(account, request).await
    }

    async fn upgrade_account_app_store(
        &self, account: &Account, request: UpgradeAccountAppStoreRequest,
    ) -> ApiResult<UpgradeAccountAppStoreRequest> {
        self.request(account, request).await
    }

    async fn cancel_subscription(
        &self, account: &Account, request: CancelSubscriptionRequest,
    ) -> ApiResult<CancelSubscriptionRequest> {
        self.request(account, request).await
    }

    async fn get_subscription_info(
        &self, account: &Account, request: GetSubscriptionInfoRequest,
    ) -> ApiResult<GetSubscriptionInfoRequest> {
        self.request(account, request).await
    }

    async fn get_subscription_info_v2(
        &self, account: &Account, request: GetSubscriptionInfoRequestV2,
    ) -> ApiResult<GetSubscriptionInfoRequestV2> {
        self.request(account, request).await
    }

    async fn upsert_debug_info(
        &self, account: &Account, request: UpsertDebugInfoRequest,
    ) -> ApiResult<UpsertDebugInfoRequest> {
        self.request(account, request).await
    }

    async fn admin_disappear_account(
        &self, account: &Account, request: AdminDisappearAccountRequest,
    ) -> ApiResult<AdminDisappearAccountRequest> {
        self.request(account, request).await
    }

    async fn admin_disappear_file(
        &self, account: &Account, request: AdminDisappearFileRequest,
    ) -> ApiResult<AdminDisappearFileRequest> {
        self.request(account, request).await
    }

    async fn admin_validate_account(
        &self, account: &Account, request: AdminValidateAccountRequest,
    ) -> ApiResult<AdminValidateAccountRequest> {
        self.request(account, request).await
    }

    async fn admin_validate_server(
        &self, account: &Account, request: AdminValidateServerRequest,
    ) -> ApiResult<AdminValidateServerRequest> {
        self.request(account, request).await
    }

    async fn admin_list_users(
        &self, account: &Account, request: AdminListUsersRequest,
    ) -> ApiResult<AdminListUsersRequest> {
        self.request(account, request).await
    }

    async fn admin_get_account_info(
        &self, account: &Account, request: AdminGetAccountInfoRequest,
    ) -> ApiResult<AdminGetAccountInfoRequest> {
        self.request(account, request).await
    }

    async fn admin_file_info(
        &self, account: &Account, request: AdminFileInfoRequest,
    ) -> ApiResult<AdminFileInfoRequest> {
        self.request(account, request).await
    }

    async fn admin_set_user_tier(
        &self, account: &Account, request: AdminSetUserTierRequest,
    ) -> ApiResult<AdminSetUserTierRequest> {
        self.request(account, request).await
    }

    async fn admin_rebuild_index(
        &self, account: &Account, request: AdminRebuildIndexRequest,
    ) -> ApiResult<AdminRebuildIndexRequest> {
        self.request(account, request).await
    }
}
