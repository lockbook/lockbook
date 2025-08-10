use crate::model::ValidationFailure;
use crate::model::account::{Account, Username};
use crate::model::crypto::*;
use crate::model::file_metadata::{DocumentHmac, FileDiff, FileMetadata, Owner};
use crate::model::signed_file::SignedFile;
use http::Method;
use libsecp256k1::PublicKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::str::FromStr;
use uuid::Uuid;

use super::server_meta::ServerMeta;
use super::signed_meta::SignedMeta;

pub const FREE_TIER_USAGE_SIZE: u64 = 25000000;
pub const PREMIUM_TIER_USAGE_SIZE: u64 = 30000000000;
/// a fee of 1000 bytes allows 1000 file creations under the free tier.
pub const METADATA_FEE: u64 = 1000;

pub trait Request: Serialize + 'static {
    type Response: Debug + DeserializeOwned + Clone;
    type Error: Debug + DeserializeOwned + Clone;
    const METHOD: Method;
    const ROUTE: &'static str;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RequestWrapper<T: Request> {
    pub signed_request: ECSigned<T>,
    pub client_version: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum ErrorWrapper<E> {
    Endpoint(E),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UpsertRequest {
    pub updates: Vec<FileDiff<SignedFile>>,
}

impl Request for UpsertRequest {
    type Response = ();
    type Error = UpsertError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/upsert-file-metadata";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum UpsertError {
    /// Arises during a call to upsert, when the caller does not have the correct old version of the
    /// File they're trying to modify
    OldVersionIncorrect,

    /// Arises during a call to upsert, when the old file is not known to the server
    OldFileNotFound,

    /// Arises during a call to upsert, when the caller suggests that a file is new, but the id already
    /// exists
    OldVersionRequired,

    /// Arises during a call to upsert, when the person making the request is not an owner of the file
    /// or has not signed the update
    NotPermissioned,

    /// Arises during a call to upsert, when a diff's new.id != old.id
    DiffMalformed,

    /// Metas in upsert cannot contain changes to digest
    HmacModificationInvalid,

    RootModificationInvalid,

    /// Found update to a deleted file
    DeletedFileUpdated,

    /// Over the User's Tier Limit
    UsageIsOverDataCap,

    /// Other misc validation failures
    Validation(ValidationFailure),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocRequest {
    pub diff: FileDiff<SignedFile>,
    pub new_content: EncryptedDocument,
}

pub struct ChangeDocRequestV2 {
    pub diff: FileDiff<SignedMeta>,
    pub new_content: EncryptedDocument,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum ChangeDocError {
    HmacMissing,
    DocumentNotFound,
    DocumentDeleted,
    NotPermissioned,
    OldVersionIncorrect,
    DiffMalformed,
    UsageIsOverDataCap,
}

impl Request for ChangeDocRequest {
    type Response = ();
    type Error = ChangeDocError;
    const METHOD: Method = Method::PUT;
    const ROUTE: &'static str = "/change-document-content";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetDocRequest {
    pub id: Uuid,
    pub hmac: DocumentHmac,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetDocumentResponse {
    pub content: EncryptedDocument,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GetDocumentError {
    DocumentNotFound,
    NotPermissioned,
}

impl Request for GetDocRequest {
    type Response = GetDocumentResponse;
    type Error = GetDocumentError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-document";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetPublicKeyRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetPublicKeyResponse {
    pub key: PublicKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GetPublicKeyError {
    InvalidUsername,
    UserNotFound,
}

impl Request for GetPublicKeyRequest {
    type Response = GetPublicKeyResponse;
    type Error = GetPublicKeyError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-public-key";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetUsernameRequest {
    pub key: PublicKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetUsernameResponse {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GetUsernameError {
    UserNotFound,
}

impl Request for GetUsernameRequest {
    type Response = GetUsernameResponse;
    type Error = GetUsernameError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-username";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetUsageRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetUsageResponse {
    pub usages: Vec<FileUsage>,
    pub cap: u64,
}

impl GetUsageResponse {
    pub fn sum_server_usage(&self) -> u64 {
        self.usages.iter().map(|usage| usage.size_bytes).sum()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub struct FileUsage {
    pub file_id: Uuid,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GetUsageError {
    UserNotFound,
}

impl Request for GetUsageRequest {
    type Response = GetUsageResponse;
    type Error = GetUsageError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-usage";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetFileIdsRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetFileIdsResponse {
    pub ids: HashSet<Uuid>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GetFileIdsError {
    UserNotFound,
}

impl Request for GetFileIdsRequest {
    type Response = GetFileIdsResponse;
    type Error = GetFileIdsError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-file-ids";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetUpdatesRequest {
    pub since_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUpdatesResponse {
    pub as_of_metadata_version: u64,
    pub file_metadata: Vec<SignedFile>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GetUpdatesError {
    UserNotFound,
}

impl Request for GetUpdatesRequest {
    type Response = GetUpdatesResponse;
    type Error = GetUpdatesError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-updates";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct NewAccountRequest {
    pub username: Username,
    pub public_key: PublicKey,
    pub root_folder: SignedFile,
}

impl NewAccountRequest {
    pub fn new(account: &Account, root_folder: &ECSigned<FileMetadata>) -> Self {
        let root_folder = root_folder.clone();
        NewAccountRequest {
            username: account.username.clone(),
            public_key: account.public_key(),
            root_folder,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct NewAccountResponse {
    pub last_synced: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum NewAccountError {
    UsernameTaken,
    PublicKeyTaken,
    InvalidUsername,
    FileIdTaken,
    Disabled,
}

impl Request for NewAccountRequest {
    type Response = NewAccountResponse;
    type Error = NewAccountError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/new-account";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetBuildInfoRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GetBuildInfoError {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetBuildInfoResponse {
    pub build_version: String,
    pub git_commit_hash: String,
}

impl Request for GetBuildInfoRequest {
    type Response = GetBuildInfoResponse;
    type Error = GetBuildInfoError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-build-info";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct DeleteAccountRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum DeleteAccountError {
    UserNotFound,
}

impl Request for DeleteAccountRequest {
    type Response = ();
    type Error = DeleteAccountError;
    const METHOD: Method = Method::DELETE;
    const ROUTE: &'static str = "/delete-account";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum PaymentMethod {
    NewCard { number: String, exp_year: i32, exp_month: i32, cvc: String },
    OldCard,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum StripeAccountTier {
    Premium(PaymentMethod),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct UpgradeAccountStripeRequest {
    pub account_tier: StripeAccountTier,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct UpgradeAccountStripeResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum UpgradeAccountStripeError {
    OldCardDoesNotExist,
    AlreadyPremium,
    CardDecline,
    InsufficientFunds,
    TryAgain,
    CardNotSupported,
    ExpiredCard,
    InvalidCardNumber,
    InvalidCardExpYear,
    InvalidCardExpMonth,
    InvalidCardCvc,
    ExistingRequestPending,
    UserNotFound,
}

impl Request for UpgradeAccountStripeRequest {
    type Response = UpgradeAccountStripeResponse;
    type Error = UpgradeAccountStripeError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/upgrade-account-stripe";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct UpgradeAccountGooglePlayRequest {
    pub purchase_token: String,
    pub account_id: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct UpgradeAccountGooglePlayResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum UpgradeAccountGooglePlayError {
    AlreadyPremium,
    InvalidPurchaseToken,
    ExistingRequestPending,
    UserNotFound,
}

impl Request for UpgradeAccountGooglePlayRequest {
    type Response = UpgradeAccountGooglePlayResponse;
    type Error = UpgradeAccountGooglePlayError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/upgrade-account-google-play";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct UpgradeAccountAppStoreRequest {
    pub original_transaction_id: String,
    pub app_account_token: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct UpgradeAccountAppStoreResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum UpgradeAccountAppStoreError {
    AppStoreAccountAlreadyLinked,
    AlreadyPremium,
    InvalidAuthDetails,
    ExistingRequestPending,
    UserNotFound,
}

impl Request for UpgradeAccountAppStoreRequest {
    type Response = UpgradeAccountAppStoreResponse;
    type Error = UpgradeAccountAppStoreError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/upgrade-account-app-store";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct CancelSubscriptionRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct CancelSubscriptionResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum CancelSubscriptionError {
    NotPremium,
    AlreadyCanceled,
    UsageIsOverFreeTierDataCap,
    UserNotFound,
    ExistingRequestPending,
    CannotCancelForAppStore,
}

impl Request for CancelSubscriptionRequest {
    type Response = CancelSubscriptionResponse;
    type Error = CancelSubscriptionError;
    const METHOD: Method = Method::DELETE;
    const ROUTE: &'static str = "/cancel-subscription";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetSubscriptionInfoRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct SubscriptionInfo {
    pub payment_platform: PaymentPlatform,
    pub period_end: UnixTimeMillis,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(tag = "tag")]
pub enum PaymentPlatform {
    Stripe { card_last_4_digits: String },
    GooglePlay { account_state: GooglePlayAccountState },
    AppStore { account_state: AppStoreAccountState },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GetSubscriptionInfoResponse {
    pub subscription_info: Option<SubscriptionInfo>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GetSubscriptionInfoError {
    UserNotFound,
}

impl Request for GetSubscriptionInfoRequest {
    type Response = GetSubscriptionInfoResponse;
    type Error = GetSubscriptionInfoError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-subscription-info";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminDisappearAccountRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminDisappearAccountError {
    NotPermissioned,
    UserNotFound,
}

impl Request for AdminDisappearAccountRequest {
    type Response = ();
    type Error = AdminDisappearAccountError;
    const METHOD: Method = Method::DELETE;
    const ROUTE: &'static str = "/admin-disappear-account";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminDisappearFileRequest {
    pub id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminDisappearFileError {
    NotPermissioned,
    FileNonexistent,
    RootModificationInvalid,
}

impl Request for AdminDisappearFileRequest {
    type Response = ();
    type Error = AdminDisappearFileError;
    const METHOD: Method = Method::DELETE;
    const ROUTE: &'static str = "/admin-disappear-file";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminValidateAccountRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct AdminValidateAccount {
    pub tree_validation_failures: Vec<ValidationFailure>,
    pub documents_missing_size: Vec<Uuid>,
    pub documents_missing_content: Vec<Uuid>,
}

impl AdminValidateAccount {
    pub fn is_empty(&self) -> bool {
        self.tree_validation_failures.is_empty()
            && self.documents_missing_content.is_empty()
            && self.documents_missing_size.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminValidateAccountError {
    NotPermissioned,
    UserNotFound,
}

impl Request for AdminValidateAccountRequest {
    type Response = AdminValidateAccount;
    type Error = AdminValidateAccountError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/admin-validate-account";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminValidateServerRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct AdminValidateServer {
    // accounts
    pub users_with_validation_failures: HashMap<Username, AdminValidateAccount>,
    // index integrity
    pub usernames_mapped_to_wrong_accounts: HashMap<String, String>,
    // mapped username -> account username
    pub usernames_mapped_to_nonexistent_accounts: HashMap<String, Owner>,
    pub usernames_unmapped_to_accounts: HashSet<String>,
    pub owners_mapped_to_unowned_files: HashMap<Owner, HashSet<Uuid>>,
    pub owners_mapped_to_nonexistent_files: HashMap<Owner, HashSet<Uuid>>,
    pub owners_unmapped_to_owned_files: HashMap<Owner, HashSet<Uuid>>,
    pub owners_unmapped: HashSet<Owner>,
    pub sharees_mapped_to_unshared_files: HashMap<Owner, HashSet<Uuid>>,
    pub sharees_mapped_to_nonexistent_files: HashMap<Owner, HashSet<Uuid>>,
    pub sharees_mapped_for_owned_files: HashMap<Owner, HashSet<Uuid>>,
    pub sharees_mapped_for_deleted_files: HashMap<Owner, HashSet<Uuid>>,
    pub sharees_unmapped_to_shared_files: HashMap<Owner, HashSet<Uuid>>,
    pub sharees_unmapped: HashSet<Owner>,
    pub files_mapped_as_parent_to_non_children: HashMap<Uuid, HashSet<Uuid>>,
    pub files_mapped_as_parent_to_nonexistent_children: HashMap<Uuid, HashSet<Uuid>>,
    pub files_mapped_as_parent_to_self: HashSet<Uuid>,
    pub files_unmapped_as_parent_to_children: HashMap<Uuid, HashSet<Uuid>>,
    pub files_unmapped_as_parent: HashSet<Uuid>,
    pub sizes_mapped_for_files_without_hmac: HashSet<Uuid>,
    pub sizes_mapped_for_nonexistent_files: HashSet<Uuid>,
    pub sizes_unmapped_for_files_with_hmac: HashSet<Uuid>,
    // document presence
    pub files_with_hmacs_and_no_contents: HashSet<Uuid>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminValidateServerError {
    NotPermissioned,
}

impl Request for AdminValidateServerRequest {
    type Response = AdminValidateServer;
    type Error = AdminValidateServerError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/admin-validate-server";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminListUsersRequest {
    pub filter: Option<AccountFilter>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AccountFilter {
    Premium,
    AppStorePremium,
    StripePremium,
    GooglePlayPremium,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminListUsersResponse {
    pub users: Vec<Username>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminListUsersError {
    NotPermissioned,
}

impl Request for AdminListUsersRequest {
    type Response = AdminListUsersResponse;
    type Error = AdminListUsersError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/admin-list-users";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminGetAccountInfoRequest {
    pub identifier: AccountIdentifier,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AccountIdentifier {
    PublicKey(PublicKey),
    Username(Username),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminGetAccountInfoResponse {
    pub account: AccountInfo,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AccountInfo {
    pub username: String,
    pub root: Uuid,
    pub payment_platform: Option<PaymentPlatform>,
    pub usage: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminGetAccountInfoError {
    UserNotFound,
    NotPermissioned,
}

impl Request for AdminGetAccountInfoRequest {
    type Response = AdminGetAccountInfoResponse;
    type Error = AdminGetAccountInfoError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/admin-get-account-info";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminFileInfoRequest {
    pub id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdminFileInfoResponse {
    pub file: ServerMeta,
    pub ancestors: Vec<ServerMeta>,
    pub descendants: Vec<ServerMeta>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminFileInfoError {
    NotPermissioned,
    FileNonexistent,
}

impl Request for AdminFileInfoRequest {
    type Response = AdminFileInfoResponse;
    type Error = AdminFileInfoError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/admin-file-info";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminSetUserTierInfo {
    Stripe {
        customer_id: String,
        customer_name: Uuid,
        payment_method_id: String,
        last_4: String,
        subscription_id: String,
        expiration_time: UnixTimeMillis,
        account_state: StripeAccountState,
    },

    GooglePlay {
        purchase_token: String,
        expiration_time: UnixTimeMillis,
        account_state: GooglePlayAccountState,
    },

    AppStore {
        account_token: String,
        original_transaction_id: String,
        expiration_time: UnixTimeMillis,
        account_state: AppStoreAccountState,
    },

    Free,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminSetUserTierRequest {
    pub username: String,
    pub info: AdminSetUserTierInfo,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdminSetUserTierResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminSetUserTierError {
    UserNotFound,
    NotPermissioned,
    ExistingRequestPending,
}

impl Request for AdminSetUserTierRequest {
    type Response = AdminSetUserTierResponse;
    type Error = AdminSetUserTierError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/admin-set-user-tier";
}

// number of milliseconds that have elapsed since the unix epoch
pub type UnixTimeMillis = u64;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Copy)]
pub enum ServerIndex {
    OwnedFiles,
    SharedFiles,
    FileChildren,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdminRebuildIndexRequest {
    pub index: ServerIndex,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AdminRebuildIndexError {
    NotPermissioned,
}

impl Request for AdminRebuildIndexRequest {
    type Response = ();
    type Error = AdminRebuildIndexError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/admin-rebuild-index";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum StripeAccountState {
    Ok,
    InvoiceFailed,
    Canceled,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GooglePlayAccountState {
    Ok,
    Canceled,
    GracePeriod,
    OnHold,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AppStoreAccountState {
    Ok,
    GracePeriod,
    FailedToRenew,
    Expired,
}

impl FromStr for StripeAccountState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Ok" => Ok(StripeAccountState::Ok),
            "Canceled" => Ok(StripeAccountState::Canceled),
            "InvoiceFailed" => Ok(StripeAccountState::InvoiceFailed),
            _ => Err(()),
        }
    }
}

impl FromStr for GooglePlayAccountState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Ok" => Ok(GooglePlayAccountState::Ok),
            "Canceled" => Ok(GooglePlayAccountState::Canceled),
            "GracePeriod" => Ok(GooglePlayAccountState::GracePeriod),
            "OnHold" => Ok(GooglePlayAccountState::OnHold),
            _ => Err(()),
        }
    }
}

impl FromStr for AppStoreAccountState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Ok" => Ok(AppStoreAccountState::Ok),
            "Expired" => Ok(AppStoreAccountState::Expired),
            "GracePeriod" => Ok(AppStoreAccountState::GracePeriod),
            "FailedToRenew" => Ok(AppStoreAccountState::FailedToRenew),
            _ => Err(()),
        }
    }
}
