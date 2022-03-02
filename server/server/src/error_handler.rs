use crate::{
    file_content_client, ClientError, GetUsageHelperError, ServerError, SimplifiedStripeError,
    StripeWebhookError,
};
use deadpool_redis::PoolError;
use lockbook_models::api::{GetUsageError, SwitchAccountTierError};
use redis::RedisError;
use redis_utils::converters::{JsonGetError, JsonSetError};
use std::fmt::Debug;
use stripe::WebhookError;

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

impl From<GetUsageHelperError> for ServerError<GetUsageError> {
    fn from(e: GetUsageHelperError) -> Self {
        match e {
            GetUsageHelperError::UserNotFound => ClientError(GetUsageError::UserNotFound),
            GetUsageHelperError::Internal(e) => ServerError::from(e),
        }
    }
}

impl From<SimplifiedStripeError> for ServerError<SwitchAccountTierError> {
    fn from(e: SimplifiedStripeError) -> Self {
        match e {
            SimplifiedStripeError::CardDeclined(decline_type) => {
                ClientError(SwitchAccountTierError::CardDeclined(decline_type))
            }
            SimplifiedStripeError::InvalidCreditCard(field) => {
                ClientError(SwitchAccountTierError::InvalidCreditCard(field))
            }
            SimplifiedStripeError::Other(msg) => internal!("{}", msg),
        }
    }
}

impl From<stripe::WebhookError> for ServerError<StripeWebhookError> {
    fn from(e: WebhookError) -> Self {
        match e {
            WebhookError::BadKey => {
                internal!("Cannot verify stripe request because server is using a bad signing key.")
            }
            WebhookError::BadHeader(bad_header_err) => {
                ClientError(StripeWebhookError::InvalidHeader(format!("{:?}", bad_header_err)))
            }
            WebhookError::BadSignature => {
                ClientError(StripeWebhookError::InvalidHeader("Bad signature.".to_string()))
            }
            WebhookError::BadTimestamp(bad_timestamp_err) => {
                ClientError(StripeWebhookError::InvalidHeader(format!(
                    "Timestamp for webhook is too old: {}",
                    bad_timestamp_err
                )))
            }
            WebhookError::BadParse(bad_parse_err) => {
                ClientError(StripeWebhookError::ParseError(format!("{:?}", bad_parse_err)))
            }
        }
    }
}
