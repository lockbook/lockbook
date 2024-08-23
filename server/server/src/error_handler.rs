use crate::account_service::DeleteAccountHelperError;
use crate::billing::billing_service::{AppStoreNotificationError, LockBillingWorkflowError};
use crate::billing::google_play_client::SimpleGCPError;
use crate::metrics::MetricsError;
use crate::ServerError::InternalError;
use crate::{
    ClientError, GetUsageHelperError, ServerError, SimplifiedStripeError, StripeWebhookError,
};
use base64::DecodeError;
use db_rs::DbError;
use jsonwebtoken::errors::ErrorKind;
use lb_rs::logic::api::*;
use lb_rs::logic::{SharedError, SharedErrorKind};
use std::fmt::Debug;
use std::io::Error;
use std::sync::PoisonError;

impl<T: Debug> From<Error> for ServerError<T> {
    fn from(err: Error) -> Self {
        internal!("IO Error: {:?}", err)
    }
}

impl<T: Debug> From<Box<bincode::ErrorKind>> for ServerError<T> {
    fn from(err: Box<bincode::ErrorKind>) -> Self {
        internal!("Bincode error: {:?}", err)
    }
}

impl<T: Debug> From<stripe::ParseIdError> for ServerError<T> {
    fn from(err: stripe::ParseIdError) -> Self {
        internal!("Stripe parse error: {:?}", err)
    }
}

impl<T: Debug> From<serde_json::Error> for ServerError<T> {
    fn from(err: serde_json::Error) -> Self {
        internal!("Serde json Error: {:?}", err)
    }
}

impl From<SharedError> for ServerError<CancelSubscriptionError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}
impl From<SharedError> for ServerError<AdminGetAccountInfoError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}
impl From<SharedError> for ServerError<GetUsageError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl From<ServerError<GetUsageHelperError>> for ServerError<GetUsageError> {
    fn from(e: ServerError<GetUsageHelperError>) -> Self {
        match e {
            ServerError::ClientError(GetUsageHelperError::UserNotFound) => {
                ClientError(GetUsageError::UserNotFound)
            }
            _ => internal!("{:?}", e),
        }
    }
}

impl From<ServerError<GetUsageHelperError>> for ServerError<CancelSubscriptionError> {
    fn from(e: ServerError<GetUsageHelperError>) -> Self {
        match e {
            ServerError::ClientError(GetUsageHelperError::UserNotFound) => {
                ClientError(CancelSubscriptionError::UserNotFound)
            }
            _ => internal!("{:?}", e),
        }
    }
}

impl From<SimpleGCPError> for ServerError<UpgradeAccountGooglePlayError> {
    fn from(e: SimpleGCPError) -> Self {
        match e {
            SimpleGCPError::PurchaseTokenNotFound => {
                ClientError(UpgradeAccountGooglePlayError::InvalidPurchaseToken)
            }
            SimpleGCPError::Unexpected(msg) => internal!("{}", msg),
        }
    }
}

impl From<SimpleGCPError> for ServerError<CancelSubscriptionError> {
    fn from(e: SimpleGCPError) -> Self {
        internal!("{:#?}", e)
    }
}

impl From<SimplifiedStripeError> for ServerError<UpgradeAccountStripeError> {
    fn from(e: SimplifiedStripeError) -> Self {
        match e {
            SimplifiedStripeError::CardDecline => {
                ClientError(UpgradeAccountStripeError::CardDecline)
            }
            SimplifiedStripeError::InsufficientFunds => {
                ClientError(UpgradeAccountStripeError::InsufficientFunds)
            }
            SimplifiedStripeError::TryAgain => ClientError(UpgradeAccountStripeError::TryAgain),
            SimplifiedStripeError::CardNotSupported => {
                ClientError(UpgradeAccountStripeError::CardNotSupported)
            }
            SimplifiedStripeError::ExpiredCard => {
                ClientError(UpgradeAccountStripeError::ExpiredCard)
            }
            SimplifiedStripeError::InvalidCardNumber => {
                ClientError(UpgradeAccountStripeError::InvalidCardNumber)
            }
            SimplifiedStripeError::InvalidCardExpYear => {
                ClientError(UpgradeAccountStripeError::InvalidCardExpYear)
            }
            SimplifiedStripeError::InvalidCardExpMonth => {
                ClientError(UpgradeAccountStripeError::InvalidCardExpMonth)
            }
            SimplifiedStripeError::InvalidCardCvc => {
                ClientError(UpgradeAccountStripeError::InvalidCardCvc)
            }
            SimplifiedStripeError::Other(msg) => internal!("{}", msg),
        }
    }
}

impl From<stripe::WebhookError> for ServerError<StripeWebhookError> {
    fn from(e: stripe::WebhookError) -> Self {
        match e {
            stripe::WebhookError::BadKey => {
                internal!("Cannot verify stripe webhook request because server is using a bad signing key")
            }
            stripe::WebhookError::BadHeader(bad_header_err) => {
                ClientError(StripeWebhookError::InvalidHeader(format!("{:?}", bad_header_err)))
            }
            stripe::WebhookError::BadSignature => {
                ClientError(StripeWebhookError::InvalidHeader("Bad signature".to_string()))
            }
            stripe::WebhookError::BadTimestamp(bad_timestamp_err) => {
                ClientError(StripeWebhookError::InvalidHeader(format!(
                    "Timestamp for webhook is too old: {}",
                    bad_timestamp_err
                )))
            }
            stripe::WebhookError::BadParse(bad_parse_err) => ClientError(
                StripeWebhookError::ParseError(format!("Parsing error: {:?}", bad_parse_err)),
            ),
        }
    }
}

impl From<jsonwebtoken::errors::Error> for ServerError<UpgradeAccountAppStoreError> {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        internal!("JWT error: {:?}", err)
    }
}

impl From<reqwest::Error> for ServerError<UpgradeAccountAppStoreError> {
    fn from(err: reqwest::Error) -> Self {
        internal!("reqwest error: {:?}", err)
    }
}

impl From<base64::DecodeError> for ServerError<AppStoreNotificationError> {
    fn from(_: DecodeError) -> Self {
        ClientError(AppStoreNotificationError::InvalidJWS)
    }
}

impl From<jsonwebtoken::errors::Error> for ServerError<AppStoreNotificationError> {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        match err.kind() {
            ErrorKind::InvalidToken
            | ErrorKind::InvalidSignature
            | ErrorKind::MissingRequiredClaim(_)
            | ErrorKind::ExpiredSignature
            | ErrorKind::InvalidIssuer
            | ErrorKind::InvalidAudience
            | ErrorKind::InvalidSubject
            | ErrorKind::ImmatureSignature
            | ErrorKind::InvalidAlgorithm
            | ErrorKind::MissingAlgorithm
            | ErrorKind::Base64(_)
            | ErrorKind::Json(_)
            | ErrorKind::Utf8(_) => ClientError(AppStoreNotificationError::InvalidJWS),
            ErrorKind::InvalidEcdsaKey
            | ErrorKind::InvalidRsaKey(_)
            | ErrorKind::RsaFailedSigning
            | ErrorKind::InvalidAlgorithmName
            | ErrorKind::InvalidKeyFormat
            | ErrorKind::Crypto(_)
            | &_ => internal!("JWT error: {:?}", err),
        }
    }
}

impl From<ServerError<LockBillingWorkflowError>> for ServerError<UpgradeAccountGooglePlayError> {
    fn from(err: ServerError<LockBillingWorkflowError>) -> Self {
        match err {
            ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
                ClientError(UpgradeAccountGooglePlayError::ExistingRequestPending)
            }
            ClientError(LockBillingWorkflowError::UserNotFound) => {
                ClientError(UpgradeAccountGooglePlayError::UserNotFound)
            }
            InternalError(msg) => InternalError(msg),
        }
    }
}

impl From<ServerError<LockBillingWorkflowError>> for ServerError<UpgradeAccountAppStoreError> {
    fn from(err: ServerError<LockBillingWorkflowError>) -> Self {
        match err {
            ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
                ClientError(UpgradeAccountAppStoreError::ExistingRequestPending)
            }
            ClientError(LockBillingWorkflowError::UserNotFound) => {
                ClientError(UpgradeAccountAppStoreError::UserNotFound)
            }
            InternalError(msg) => InternalError(msg),
        }
    }
}

impl From<ServerError<LockBillingWorkflowError>> for ServerError<UpgradeAccountStripeError> {
    fn from(err: ServerError<LockBillingWorkflowError>) -> Self {
        match err {
            ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
                ClientError(UpgradeAccountStripeError::ExistingRequestPending)
            }
            ClientError(LockBillingWorkflowError::UserNotFound) => {
                ClientError(UpgradeAccountStripeError::UserNotFound)
            }
            InternalError(msg) => InternalError(msg),
        }
    }
}

impl From<ServerError<LockBillingWorkflowError>> for ServerError<CancelSubscriptionError> {
    fn from(err: ServerError<LockBillingWorkflowError>) -> Self {
        match err {
            ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
                ClientError(CancelSubscriptionError::ExistingRequestPending)
            }
            ClientError(LockBillingWorkflowError::UserNotFound) => {
                ClientError(CancelSubscriptionError::UserNotFound)
            }
            InternalError(msg) => InternalError(msg),
        }
    }
}

impl From<ServerError<LockBillingWorkflowError>> for ServerError<AdminSetUserTierError> {
    fn from(err: ServerError<LockBillingWorkflowError>) -> Self {
        match err {
            ClientError(LockBillingWorkflowError::ExistingRequestPending) => {
                ClientError(AdminSetUserTierError::ExistingRequestPending)
            }
            ClientError(LockBillingWorkflowError::UserNotFound) => {
                ClientError(AdminSetUserTierError::UserNotFound)
            }
            InternalError(msg) => InternalError(msg),
        }
    }
}

impl From<SharedError> for ServerError<DeleteAccountHelperError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl From<ServerError<DeleteAccountHelperError>> for ServerError<DeleteAccountError> {
    fn from(err: ServerError<DeleteAccountHelperError>) -> Self {
        match err {
            ClientError(DeleteAccountHelperError::UserNotFound) => {
                ClientError(DeleteAccountError::UserNotFound)
            }
            InternalError(msg) => InternalError(msg),
        }
    }
}

impl From<ServerError<DeleteAccountHelperError>> for ServerError<AdminDisappearAccountError> {
    fn from(err: ServerError<DeleteAccountHelperError>) -> Self {
        match err {
            ClientError(DeleteAccountHelperError::UserNotFound) => {
                ClientError(AdminDisappearAccountError::UserNotFound)
            }
            InternalError(msg) => InternalError(msg),
        }
    }
}

impl From<SharedError> for ServerError<AdminValidateAccountError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl From<SharedError> for ServerError<AdminValidateServerError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl From<SharedError> for ServerError<AdminFileInfoError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl From<SharedError> for ServerError<AdminDisappearFileError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl From<SharedError> for ServerError<UpsertError> {
    fn from(err: SharedError) -> Self {
        // panic!("{err}");
        use lb_rs::logic::api::UpsertError::*;
        match err.kind {
            SharedErrorKind::OldVersionIncorrect => ClientError(OldVersionIncorrect),
            SharedErrorKind::OldFileNotFound => ClientError(OldFileNotFound),
            SharedErrorKind::OldVersionRequired => ClientError(OldVersionRequired),
            SharedErrorKind::InsufficientPermission => ClientError(NotPermissioned),
            SharedErrorKind::DiffMalformed => ClientError(DiffMalformed),
            SharedErrorKind::HmacModificationInvalid => ClientError(HmacModificationInvalid),
            SharedErrorKind::DeletedFileUpdated(_) => ClientError(DeletedFileUpdated),
            SharedErrorKind::RootModificationInvalid => ClientError(RootModificationInvalid),
            SharedErrorKind::ValidationFailure(fail) => ClientError(Validation(fail)),
            SharedErrorKind::Unexpected(msg) => InternalError(String::from(msg)),
            _ => internal!("{:?}", err),
        }
    }
}

impl From<SharedError> for ServerError<ChangeDocError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl From<SharedError> for ServerError<MetricsError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl From<SharedError> for ServerError<GetDocumentError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl From<SharedError> for ServerError<GetFileIdsError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl From<SharedError> for ServerError<GetUpdatesError> {
    fn from(err: SharedError) -> Self {
        internal!("{:?}", err)
    }
}

impl<T: Debug> From<DbError> for ServerError<T> {
    fn from(value: DbError) -> Self {
        internal!("db-rs error {:?}", value)
    }
}

impl<T: Debug, G> From<PoisonError<G>> for ServerError<T> {
    fn from(value: PoisonError<G>) -> Self {
        internal!("mutex poisoned {:?}", value)
    }
}
