use serde::{Deserialize, Serialize};

// A stripe request can either be your expected struct, or the StripeErrorContainer
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
#[serde(rename_all = "snake_case")]
pub enum StripeErrorType {
    ApiError,
    CardError,
    IdempotencyError,
    InvalidRequestError,
}

// Since certain fields can either be a String/Int or a Struct, I made this to handle either situation
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum StripeMaybeContainer<K, U> {
    Expected(K),
    Unexpected(U),
}

// Not all of stripe's defined error codes are bellow, these are the ones I am expecting to be returned.
// The unexpected error codes will be caught with StripeMaybeContainer magic.
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

// Similar to the comment above
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeKnownErrorDeclineCode {
    ApproveWithId,
    CallIssuer,
    CardNotSupported,
    CardVelocityExceeded,
    CurrencyNotSupported,
    DoNotHonor,
    DoNotTryAgain,
    ExpiredCard,
    Fraudulent,
    GenericDecline,
    IncorrectNumber,
    IncorrectCvc,
    InsufficientFunds,
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
    TransactionNotAllowed,
    TryAgainLater,
    WithdrawalCountLimitExceeded,
}

// Practically every stripe response contains an id field, and in some cases that is all that is needed.
#[derive(Serialize, Deserialize)]
pub struct BasicStripeResponse {
    pub id: String,
}

// The following structs with the suffix "Response" can entirely be their own returns from stripe.

#[derive(Debug, Serialize, Deserialize)]
pub struct StripeSubscriptionResponse {
    pub id: String,
    pub status: SubscriptionStatus,
    pub latest_invoice: StripeInvoiceResponse,
    pub current_period_end: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripeInvoiceResponse {
    pub id: String,
    pub payment_intent: StripeMaybeContainer<StripePaymentIntentResponse, String>,
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
pub struct StripePaymentIntentResponse {
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
    pub created: u64,
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
pub struct StripeWebhookResponse {
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
    Invoice(Box<StripeInvoiceResponse>),
    Unmatched(serde_json::Value),
}

// These structs are stored on redis
pub const UNSET_CUSTOMER_ID: &str = "not_st";

#[derive(Serialize, Deserialize, Debug)]
pub struct StripeUserInfo {
    pub customer_id: String,
    pub payment_methods: Vec<StripePaymentInfo>,
    pub subscriptions: Vec<StripeSubscriptionInfo>,
}

impl Default for StripeUserInfo {
    fn default() -> Self {
        StripeUserInfo {
            customer_id: UNSET_CUSTOMER_ID.to_string(),
            payment_methods: vec![],
            subscriptions: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StripePaymentInfo {
    pub id: String,
    pub last_4: String,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StripeSubscriptionInfo {
    pub id: String,
    pub period_end: u64,
    pub is_active: bool,
}
