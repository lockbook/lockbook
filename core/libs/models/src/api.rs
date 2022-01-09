use libsecp256k1::PublicKey;
use reqwest::Method;
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
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new(metadata)],
        }
    }

    pub fn new_diff(
        old_parent: Uuid,
        old_name: &SecretFileName,
        new_metadata: &EncryptedFileMetadata,
    ) -> Self {
        FileMetadataUpsertsRequest {
            updates: vec![FileMetadataDiff::new_diff(
                old_parent,
                old_name,
                new_metadata,
            )],
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
    InvalidUsername,
    NotPermissioned,
    UserNotFound,
    DocumentNotFound,
    EditConflict,
    DocumentDeleted,
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
    InvalidUsername,
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
    InvalidUsername,
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
    pub folder_id: Uuid,
    pub folder_name: SecretFileName,
    pub parent_access_key: EncryptedFolderAccessKey,
    pub user_access_key: EncryptedUserAccessKey,
}

impl NewAccountRequest {
    pub fn new(account: &Account, root_metadata: &EncryptedFileMetadata) -> Self {
        NewAccountRequest {
            username: account.username.clone(),
            public_key: account.public_key(),
            folder_id: root_metadata.id,
            folder_name: root_metadata.name.clone(),
            parent_access_key: root_metadata.folder_access_keys.clone(),
            user_access_key: root_metadata
                .user_access_keys
                .get(&account.username)
                .expect("file metadata for new account request must have user access key") // TODO: handle better
                .access_key
                .clone(),
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
    InvalidPublicKey,
    InvalidUserAccessKey,
    InvalidUsername,
    FileIdTaken,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreditCardInfo {
    pub payment_method_id: String,
    pub last_4_digits: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetLastRegisteredCreditCardRequest {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetLastRegisteredCreditCardResponse {
    pub credit_card: CreditCardInfo,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum GetLastRegisteredCreditCardError {}

impl Request for GetLastRegisteredCreditCardRequest {
    type Response = GetLastRegisteredCreditCardResponse;
    type Error = GetLastRegisteredCreditCardError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/get-registered-credit-cards";
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum CardChoice {
    NewCard {
        number: String,
        exp_year: String,
        exp_month: String,
        cvc: String,
    },
    OldCard,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum AccountTier {
    Monthly(CardChoice),
    Free,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SwitchAccountTierRequest {
    pub account_tier: AccountTier,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SwitchAccountTierResponse {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum InvalidCreditCardType {
    Number,
    ExpYear,
    ExpMonth,
    CVC,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum CardDeclinedType {
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
    PreexistingCardDoesNotExist,
    NewTierIsOldTier,
    CardDeclined(CardDeclinedType),
    InvalidCreditCard(InvalidCreditCardType),
}

impl Request for SwitchAccountTierRequest {
    type Response = SwitchAccountTierResponse;
    type Error = SwitchAccountTierError;
    const METHOD: Method = Method::POST;
    const ROUTE: &'static str = "/switch-account-tier";
}
