use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StripeResult<U> {
    Ok(U),
    Err(StripeErrorContainer),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripeErrorContainer {
    pub error: StripeError,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripeError {
    #[serde(rename = "type")]
    pub error_type: StripeErrorType,
    pub code: StripeMaybeContainer<StripeKnownErrorCode, String>,
    pub decline_code: Option<StripeMaybeContainer<StripeKnownErrorDeclineCode, String>>,
    pub doc_url: String,
    pub message: String,
    pub param: String,
    pub payment_method_type: Option<String>,
    pub charge: Option<String>,
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
    InvalidRequestError,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum StripeMaybeContainer<K, U> {
    Expected(K),
    Unexpected(U),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeKnownErrorCode {
    CardDeclineRateLimitExceeded,
    CardDeclined,
    ExpiredCard,
    IncorrectCvc,
    IncorrectNumber,
    InsufficientFunds,
    InvalidCvc,
    InvalidExpiryMonth,
    InvalidExpiryYear,
    InvalidNumber,
    PaymentIntentAuthenticationFailure,
    ProcessingError,
    SetupIntentAuthenticationFailure,
}
// customer_max_payment_methods

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeKnownErrorDeclineCode {
    // AuthenticationRequired,
    ApproveWithId,
    CallIssuer,
    CardNotSupported,
    CardVelocityExceeded,
    CurrencyNotSupported,
    DoNotHonor,
    DoNotTryAgain,
    // DuplicateTransaction,
    ExpiredCard,
    Fraudulent,
    GenericDecline,
    IncorrectNumber,
    IncorrectCvc,
    // IncorrectPin,
    InsufficientFunds,
    // InvalidAccount,
    // InvalidAmount,
    InvalidCvc,
    InvalidExpiryMonth,
    InvalidExpiryYear,
    InvalidNumber,
    IssuerNotAvailable,
    LostCard,
    MerchantBlacklist,
    NewAccountInformationAvailable,
    NoActionTaken,
    NotPermitted,
    PickupCard,
    ProcessingError,
    ReenterTransaction,
    RestrictedCard,
    RevocationOfAllAuthorizations,
    RevocationOfAuthorization,
    SecurityViolation,
    ServiceNotAllowed,
    StolenCard,
    StopPaymentOrder,
    // TestmodeDecline,
    TransactionNotAllowed,
    TryAgainLater,
    WithdrawalCountLimitExceeded,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripeSubscriptionResponse {
    pub id: String,
    pub status: SubscriptionStatus,
    pub latest_invoice: StripeInvoice,
    pub current_period_end: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripeInvoice {
    pub id: String,
    pub payment_intent: StripeMaybeContainer<StripePaymentIntent, String>,
    pub subscription: StripeMaybeContainer<Box<StripeSubscriptionResponse>, String>,
    #[serde(rename = "customer")]
    pub customer_id: String,
    pub billing_reason: StripeBillingReason,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeBillingReason {
    SubscriptionCycle,
    SubscriptionCreate,
    SubscriptionUpdate,
    Subscription,
    Manual,
    Upcoming,
    SubscriptionThreshold,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripePaymentIntent {
    pub status: SetupPaymentIntentStatus,
    pub last_payment_error: Option<StripeError>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Incomplete,
    IncompleteExpired,
    Trialing,
    Active,
    PastDue,
    Canceled,
    Unpaid,
}

#[derive(Serialize, Deserialize)]
pub struct BasicStripeResponse {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SetupPaymentIntentStatus {
    Succeeded,
    RequiresAction,
    RequiresPaymentMethod,
}

#[derive(Serialize, Deserialize)]
pub struct StripePaymentMethodResponse {
    pub id: String,
    pub card: PaymentMethodCard,
}

#[derive(Serialize, Deserialize)]
pub struct PaymentMethodCard {
    pub last4: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripeSetupIntentResponse {
    pub status: SetupPaymentIntentStatus,
    pub last_setup_error: Option<StripeError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripeWebhook {
    #[serde(rename = "type")]
    pub event_type: StripeMaybeContainer<StripeEventType, String>,
    pub data: StripeEventObjectContainer,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum StripeEventType {
    #[serde(rename = "invoice.paid")]
    InvoicePaid,
    #[serde(rename = "invoice.payment_failed")]
    InvoicePaymentFailed,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StripeEventObjectContainer {
    pub object: StripeObjectType,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum StripeObjectType {
    Invoice(StripeInvoice),
    Unmatched(serde_json::Value),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StripeEmptyRequest {}
