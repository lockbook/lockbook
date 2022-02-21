use serde::{Deserialize, Serialize};

// This struct provides a fall back to unknown decline codes from Stripe. Since decline codes aren't parsed by "async-stripe" (a crate),
// we provide our own solution to parse them. Although, there are instances in which we may receive an unknown decline code. Rather than
// making serde handle this and return an internal error, we provide this method to catch the code.
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum StripeDeclineCodeCatcher {
    Known(StripeKnownDeclineCode),
    Unknown(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeKnownDeclineCode {
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

// This is the stripe information for a single lockbook user. This is stored on redis.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct StripeUserInfo {
    pub customer_id: Option<String>,
    pub payment_methods: Vec<StripePaymentInfo>,
    pub subscriptions: Vec<StripeSubscriptionInfo>,
    pub last_in_payment_flow: i64,
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
