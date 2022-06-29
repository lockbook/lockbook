use crate::billing::billing_service::LockBillingWorkflowError;
use crate::billing::google_play_client::SimpleGCPError;
use crate::ServerError::InternalError;
use crate::{
    ClientError, GetUsageHelperError, ServerError, SimplifiedStripeError, StripeWebhookError,
};
use lockbook_models::api::{
    CancelSubscriptionError, GetUsageError, UpgradeAccountGooglePlayError,
    UpgradeAccountStripeError,
};
use std::fmt::Debug;
use std::io::Error;

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

impl From<GetUsageHelperError> for ServerError<GetUsageError> {
    fn from(e: GetUsageHelperError) -> Self {
        match e {
            GetUsageHelperError::UserNotFound => ClientError(GetUsageError::UserNotFound),
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
                internal!("Cannot verify stripe webhook request because server is using a bad signing key.")
            }
            stripe::WebhookError::BadHeader(bad_header_err) => {
                ClientError(StripeWebhookError::InvalidHeader(format!("{:?}", bad_header_err)))
            }
            stripe::WebhookError::BadSignature => {
                ClientError(StripeWebhookError::InvalidHeader("Bad signature.".to_string()))
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
