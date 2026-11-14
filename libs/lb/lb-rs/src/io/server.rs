use std::future::Future;

use crate::io::network::ApiError;
use crate::model::account::Account;
use crate::model::api::*;

/// What a request of type `T` resolves to.
pub type ApiResult<T> = Result<<T as Request>::Response, ApiError<<T as Request>::Error>>;

/// `Send` on native targets, where lb-rs spawns onto tokio's thread pool; nothing on wasm, whose
/// HTTP futures are single-threaded.
#[cfg(not(target_family = "wasm"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_family = "wasm"))]
impl<T: Send> MaybeSend for T {}

#[cfg(target_family = "wasm")]
pub trait MaybeSend {}
#[cfg(target_family = "wasm")]
impl<T> MaybeSend for T {}

/// `Sync` where [MaybeSend] is `Send`.
#[cfg(not(target_family = "wasm"))]
pub trait MaybeSync: Sync {}
#[cfg(not(target_family = "wasm"))]
impl<T: Sync> MaybeSync for T {}

#[cfg(target_family = "wasm")]
pub trait MaybeSync {}
#[cfg(target_family = "wasm")]
impl<T> MaybeSync for T {}

/// Lockbook's server as lb-rs reaches it: one method per [Request] in [crate::model::api].
pub trait Server: Clone + MaybeSend + MaybeSync + 'static {
    fn new_account_v2(
        &self, account: &Account, request: NewAccountRequestV2,
    ) -> impl Future<Output = ApiResult<NewAccountRequestV2>> + MaybeSend;

    fn new_sub_account(
        &self, account: &Account, request: NewSubAccountRequest,
    ) -> impl Future<Output = ApiResult<NewSubAccountRequest>> + MaybeSend;

    fn list_sub_accounts(
        &self, account: &Account, request: ListSubAccountsRequest,
    ) -> impl Future<Output = ApiResult<ListSubAccountsRequest>> + MaybeSend;

    fn delete_sub_account(
        &self, account: &Account, request: DeleteSubAccountRequest,
    ) -> impl Future<Output = ApiResult<DeleteSubAccountRequest>> + MaybeSend;

    fn change_doc_v2(
        &self, account: &Account, request: ChangeDocRequestV2,
    ) -> impl Future<Output = ApiResult<ChangeDocRequestV2>> + MaybeSend;

    fn upsert_file_metadata_v2(
        &self, account: &Account, request: UpsertRequestV2,
    ) -> impl Future<Output = ApiResult<UpsertRequestV2>> + MaybeSend;

    fn get_document(
        &self, account: &Account, request: GetDocRequest,
    ) -> impl Future<Output = ApiResult<GetDocRequest>> + MaybeSend;

    fn get_public_key(
        &self, account: &Account, request: GetPublicKeyRequest,
    ) -> impl Future<Output = ApiResult<GetPublicKeyRequest>> + MaybeSend;

    fn get_username(
        &self, account: &Account, request: GetUsernameRequest,
    ) -> impl Future<Output = ApiResult<GetUsernameRequest>> + MaybeSend;

    fn get_usage(
        &self, account: &Account, request: GetUsageRequest,
    ) -> impl Future<Output = ApiResult<GetUsageRequest>> + MaybeSend;

    fn get_file_ids(
        &self, account: &Account, request: GetFileIdsRequest,
    ) -> impl Future<Output = ApiResult<GetFileIdsRequest>> + MaybeSend;

    fn get_updates_v2(
        &self, account: &Account, request: GetUpdatesRequestV2,
    ) -> impl Future<Output = ApiResult<GetUpdatesRequestV2>> + MaybeSend;

    fn get_build_info(
        &self, account: &Account, request: GetBuildInfoRequest,
    ) -> impl Future<Output = ApiResult<GetBuildInfoRequest>> + MaybeSend;

    fn delete_account(
        &self, account: &Account, request: DeleteAccountRequest,
    ) -> impl Future<Output = ApiResult<DeleteAccountRequest>> + MaybeSend;

    fn upgrade_account_stripe(
        &self, account: &Account, request: UpgradeAccountStripeRequest,
    ) -> impl Future<Output = ApiResult<UpgradeAccountStripeRequest>> + MaybeSend;

    fn upgrade_account_google_play(
        &self, account: &Account, request: UpgradeAccountGooglePlayRequest,
    ) -> impl Future<Output = ApiResult<UpgradeAccountGooglePlayRequest>> + MaybeSend;

    fn upgrade_account_app_store(
        &self, account: &Account, request: UpgradeAccountAppStoreRequest,
    ) -> impl Future<Output = ApiResult<UpgradeAccountAppStoreRequest>> + MaybeSend;

    fn cancel_subscription(
        &self, account: &Account, request: CancelSubscriptionRequest,
    ) -> impl Future<Output = ApiResult<CancelSubscriptionRequest>> + MaybeSend;

    fn get_subscription_info(
        &self, account: &Account, request: GetSubscriptionInfoRequest,
    ) -> impl Future<Output = ApiResult<GetSubscriptionInfoRequest>> + MaybeSend;

    fn get_subscription_info_v2(
        &self, account: &Account, request: GetSubscriptionInfoRequestV2,
    ) -> impl Future<Output = ApiResult<GetSubscriptionInfoRequestV2>> + MaybeSend;

    fn upsert_debug_info(
        &self, account: &Account, request: UpsertDebugInfoRequest,
    ) -> impl Future<Output = ApiResult<UpsertDebugInfoRequest>> + MaybeSend;

    fn admin_disappear_account(
        &self, account: &Account, request: AdminDisappearAccountRequest,
    ) -> impl Future<Output = ApiResult<AdminDisappearAccountRequest>> + MaybeSend;

    fn admin_disappear_file(
        &self, account: &Account, request: AdminDisappearFileRequest,
    ) -> impl Future<Output = ApiResult<AdminDisappearFileRequest>> + MaybeSend;

    fn admin_validate_account(
        &self, account: &Account, request: AdminValidateAccountRequest,
    ) -> impl Future<Output = ApiResult<AdminValidateAccountRequest>> + MaybeSend;

    fn admin_validate_server(
        &self, account: &Account, request: AdminValidateServerRequest,
    ) -> impl Future<Output = ApiResult<AdminValidateServerRequest>> + MaybeSend;

    fn admin_list_users(
        &self, account: &Account, request: AdminListUsersRequest,
    ) -> impl Future<Output = ApiResult<AdminListUsersRequest>> + MaybeSend;

    fn admin_get_account_info(
        &self, account: &Account, request: AdminGetAccountInfoRequest,
    ) -> impl Future<Output = ApiResult<AdminGetAccountInfoRequest>> + MaybeSend;

    fn admin_file_info(
        &self, account: &Account, request: AdminFileInfoRequest,
    ) -> impl Future<Output = ApiResult<AdminFileInfoRequest>> + MaybeSend;

    fn admin_set_user_tier(
        &self, account: &Account, request: AdminSetUserTierRequest,
    ) -> impl Future<Output = ApiResult<AdminSetUserTierRequest>> + MaybeSend;

    fn admin_rebuild_index(
        &self, account: &Account, request: AdminRebuildIndexRequest,
    ) -> impl Future<Output = ApiResult<AdminRebuildIndexRequest>> + MaybeSend;
}
