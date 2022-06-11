use crate::billing::google_play_client::SimpleGCPError;
use crate::{
    file_content_client, ClientError, GetUsageHelperError, ServerError, SimplifiedStripeError,
    StripeWebhookError,
};
use deadpool_redis::PoolError;
use lockbook_models::api::{
    CancelSubscriptionError, GetUsageError, UpgradeAccountGooglePlayError, UpgradeAccountStripeError,
};
use redis::RedisError;
use redis_utils::converters::{JsonGetError, JsonSetError};
use std::fmt::Debug;

impl<T: Debug> From<PoolError> for ServerError<T> {
    fn from(err: PoolError) -> Self {
        internal!("Could not get connection for pool: {:?}", err)
    }
}

impl<T: Debug> From<RedisError> for ServerError<T> {
    fn from(err: RedisError) -> Self {
        internal!("Redis Error: {:?}", err)
    }
}

impl<T: Debug> From<JsonGetError> for ServerError<T> {
    fn from(err: JsonGetError) -> Self {
        internal!("Redis Error: {:?}", err)
    }
}

impl<T: Debug> From<JsonSetError> for ServerError<T> {
    fn from(err: JsonSetError) -> Self {
        internal!("Redis Error: {:?}", err)
    }
}

impl<T: Debug> From<file_content_client::Error> for ServerError<T> {
    fn from(err: file_content_client::Error) -> Self {
        internal!("S3 Error: {:?}", err)
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
            GetUsageHelperError::Internal(e) => ServerError::from(e),
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
