use http::Method;
use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::account::Account;
use crate::account::Username;
use crate::crypto::*;
use crate::file_metadata::{EncryptedFileMetadata, FileMetadataDiff};

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
    pub fn new(metadata: &EncryptedFileMetadata) -> Self {
        FileMetadataUpsertsRequest { updates: vec![FileMetadataDiff::new(metadata)] }
    }

    pub fn new_diff(
        old_parent: Uuid, old_name: &SecretFileName, new_metadata: &EncryptedFileMetadata,
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum FileMetadataUpsertsError {
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetDocumentResponse {
    pub content: Option<EncryptedDocument>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetDocumentError {
    DocumentNotFound,
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
    pub file_metadata: Vec<EncryptedFileMetadata>,
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
    pub root_folder: EncryptedFileMetadata,
}

impl NewAccountRequest {
    pub fn new(account: &Account, root_folder: &EncryptedFileMetadata) -> Self {
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
pub struct GetCreditCardRequest {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetCreditCardResponse {
    pub credit_card_last_4_digits: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetCreditCardError {
    NotAStripeCustomer,
}

impl Request for GetCreditCardRequest {
    type Response = GetCreditCardResponse;
    type Error = GetCreditCardError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/get-credit-card";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum PaymentMethod {
    NewCard { number: String, exp_year: i32, exp_month: i32, cvc: String },
    OldCard,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum AccountTier {
    Monthly(PaymentMethod),
    Free,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SwitchAccountTierRequest {
    pub account_tier: AccountTier,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SwitchAccountTierResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum CreditCardRejectReason {
    Number,
    ExpYear,
    ExpMonth,
    CVC,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum CardDeclineReason {
    Generic,
    BalanceOrCreditExceeded,
    TooManyTries,
    Unknown,
    TryAgain,
    NotSupported,
    ExpiredCard,
    IncorrectNumber,
    IncorrectCVC,
    IncorrectExpiryMonth,
    IncorrectExpiryYear,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum SwitchAccountTierError {
    OldCardDoesNotExist,
    NewTierIsOldTier,
    CardDeclined(CardDeclineReason),
    InvalidCreditCard(CreditCardRejectReason),
    CurrentUsageIsMoreThanNewTier,
    CurrentlyInBillingWorkflow,
    UserNotFound,
}

impl Request for SwitchAccountTierRequest {
    type Response = SwitchAccountTierResponse;
    type Error = SwitchAccountTierError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/switch-account-tier";
}
