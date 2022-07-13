use http::Method;
use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::account::Account;
use crate::account::Username;
use crate::crypto::*;
use crate::file_metadata::{FileMetadataDiff, UnsignedFile};
use crate::tree::FileLike;

pub trait Request {
    type Response;
    type Error;
    const METHOD: Method;
    const ROUTE: &'static str;
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RequestWrapper<T: Request> {
    pub signed_request: ECSigned<T>,
    pub client_version: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ErrorWrapper<E> {
    Endpoint(E),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FileMetadataUpsertsRequest {
    pub updates: Vec<FileMetadataDiff>,
}

impl FileMetadataUpsertsRequest {
    pub fn new(metadata: &UnsignedFile) -> Self {
        FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(metadata)] }
    }

    pub fn new_diff(
        old_parent: Uuid, old_name: &SecretFileName, new_metadata: &UnsignedFile,
    ) -> Self {
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new_diff(old_parent, old_name, new_metadata)],
        }
    }
}

impl Request for FileMetadataUpsertsRequest {
    type Response = ();
    type Error = FileMetadataUpsertsError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/upsert-file-metadata";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum FileMetadataUpsertsError {
    NotPermissioned,
    NewFileHasOldParentAndName,
    NewIdAlreadyExists,
    UserNotFound,
    RootImmutable,
    GetUpdatesRequired,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocumentContentRequest {
    pub id: Uuid,
    pub old_metadata_version: u64,
    pub new_content: EncryptedDocument,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChangeDocumentContentResponse {
    pub new_content_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ChangeDocumentContentError {
    DocumentNotFound,
    DocumentDeleted,
    NotPermissioned,
    EditConflict,
}

impl Request for ChangeDocumentContentRequest {
    type Response = ChangeDocumentContentResponse;
    type Error = ChangeDocumentContentError;
    const METHOD: Method = Method::PUT;
    const ROUTE: &'static str = "/change-document-content";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetDocumentRequest {
    pub id: Uuid,
    pub content_version: u64,
}

impl<F> From<&F> for GetDocumentRequest
where
    F: FileLike,
{
    fn from(meta: &F) -> Self {
        Self { id: meta.id(), content_version: meta.content_version() }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetDocumentResponse {
    pub content: EncryptedDocument,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetDocumentError {
    DocumentNotFound,
    NotPermissioned,
}

impl Request for GetDocumentRequest {
    type Response = GetDocumentResponse;
    type Error = GetDocumentError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-document";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetPublicKeyRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetPublicKeyResponse {
    pub key: PublicKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUsageRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUsageResponse {
    pub usages: Vec<FileUsage>,
    pub cap: u64,
}

impl GetUsageResponse {
    pub fn sum_server_usage(&self) -> u64 {
        self.usages.iter().map(|usage| usage.size_bytes).sum()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FileUsage {
    pub file_id: Uuid,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetUsageError {
    UserNotFound,
}

impl Request for GetUsageRequest {
    type Response = GetUsageResponse;
    type Error = GetUsageError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-usage";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUpdatesRequest {
    pub since_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetUpdatesResponse {
    pub file_metadata: Vec<UnsignedFile>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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
    pub root_folder: UnsignedFile,
}

impl NewAccountRequest {
    pub fn new(account: &Account, root_folder: &UnsignedFile) -> Self {
        let root_folder = root_folder.clone();
        NewAccountRequest {
            username: account.username.clone(),
            public_key: account.public_key(),
            root_folder,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct NewAccountResponse {
    pub folder_metadata_version: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetBuildInfoRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetBuildInfoError {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteAccountRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum DeleteAccountError {
    UserNotFound,
}

impl Request for DeleteAccountRequest {
    type Response = ();
    type Error = DeleteAccountError;
    const METHOD: Method = Method::DELETE;
    const ROUTE: &'static str = "/delete-account";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum PaymentMethod {
    NewCard { number: String, exp_year: i32, exp_month: i32, cvc: String },
    OldCard,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum StripeAccountTier {
    Premium(PaymentMethod),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UpgradeAccountStripeRequest {
    pub account_tier: StripeAccountTier,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UpgradeAccountStripeResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UpgradeAccountGooglePlayRequest {
    pub purchase_token: String,
    pub account_id: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UpgradeAccountGooglePlayResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CancelSubscriptionRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CancelSubscriptionResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetSubscriptionInfoRequest {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SubscriptionInfo {
    pub payment_platform: PaymentPlatform,
    pub period_end: UnixTimeMillis,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(tag = "tag")]
pub enum PaymentPlatform {
    Stripe { card_last_4_digits: String },
    GooglePlay { account_state: GooglePlayAccountState },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GooglePlayAccountState {
    Ok,
    Canceled,
    GracePeriod,
    OnHold,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetSubscriptionInfoResponse {
    pub subscription_info: Option<SubscriptionInfo>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetSubscriptionInfoError {
    UserNotFound,
}

impl Request for GetSubscriptionInfoRequest {
    type Response = GetSubscriptionInfoResponse;
    type Error = GetSubscriptionInfoError;
    const METHOD: Method = Method::GET;
    const ROUTE: &'static str = "/get-subscription-info";
}

// number of milliseconds that have elapsed since the unix epoch
pub type UnixTimeMillis = u64;
