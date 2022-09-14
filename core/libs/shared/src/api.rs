use http::Method;
use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::account::Account;
use crate::account::Username;
use crate::crypto::*;
use crate::file_metadata::{DocumentHmac, FileDiff, FileMetadata};
use crate::signed_file::SignedFile;
use crate::ValidationFailure;

pub trait Request {
    type Response;
    type Error;
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

    /// Other misc validation failures
    Validation(ValidationFailure),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocRequest {
    pub diff: FileDiff<SignedFile>,
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
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
    pub build_version: &'static str,
    pub git_commit_hash: &'static str,
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
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GooglePlayAccountState {
    Ok,
    Canceled,
    GracePeriod,
    OnHold,
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
pub struct AdminDeleteAccountRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminDeleteAccountError {
    NotPermissioned,
    UserNotFound,
}

impl Request for AdminDeleteAccountRequest {
    type Response = ();
    type Error = AdminDeleteAccountError;
    const METHOD: Method = Method::DELETE;
    const ROUTE: &'static str = "/admin-delete-account";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminDisappearFileRequest {
    pub id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminDisappearFileError {
    NotPermissioned,
    FileNonexistent,
}

impl Request for AdminDisappearFileRequest {
    type Response = ();
    type Error = AdminDisappearFileError;
    const METHOD: Method = Method::DELETE;
    const ROUTE: &'static str = "/admin-disappear-file";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminServerValidateRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AdminServerValidateResponse {
    pub tree_validation_failures: Vec<ValidationFailure>,
    pub documents_missing_size: Vec<Uuid>,
    pub documents_missing_content: Vec<Uuid>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AdminServerValidateError {
    NotPermissioned,
    UserNotFound,
}

impl Request for AdminServerValidateRequest {
    type Response = AdminServerValidateResponse;
    type Error = AdminServerValidateError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/admin-server-validate";
}

// number of milliseconds that have elapsed since the unix epoch
pub type UnixTimeMillis = u64;
