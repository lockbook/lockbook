use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StripeResult<U> {
    Ok(U),
    Err(StripeErrorContainer)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripeErrorContainer {
    error: StripeError
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripeError {
    #[serde(rename = "type")]
    error_type: StripeErrorType,
    code: StripeErrorCode,
    doc_url: String,
    message: String,
    param: String,
    payment_method_type: Option<String>,
    charge: Option<String>,
    decline_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StripeErrorType {
    #[serde(rename = "api_error")]
    ApiError,
    #[serde(rename = "card_error")]
    CardError,
    #[serde(rename = "idempotency_error")]
    IdempotencyError,
    #[serde(rename = "invalid_request_error")]
    InvalidRequestError
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StripeErrorCode {
    Known(StripeKnownErrorCode),
    Unknown(String)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeKnownErrorCode {
    InvalidCVC,
    InvalidExpiryMonth,
    InvalidExpiryYear,
    InvalidNumber,
    CardDeclineRateLimitExceeded,
    CardDeclined,
    DebitNotAuthorized,
    ExpiredCard,
    IncorrectNumber
}

#[derive(Serialize, Deserialize)]
pub struct BasicStripeResponse {
    pub id: String,
}

#[derive(Serialize, Deserialize)]
pub struct SetupIntentStripeResponse {
    pub status: String,
}

#[derive(Serialize, Deserialize)]
pub struct PaymentMethodStripeResponse {
    pub id: String,
    pub card: PaymentMethodCard,
}

#[derive(Serialize, Deserialize)]
pub struct PaymentMethodCard {
    pub last4: String,
}
