use serde::{Deserialize, Serialize};

// Since certain fields can either be a String/Int or a Struct, I made this to handle either situation
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum StripeMaybeContainer<K, U> {
    Expected(K),
    Unexpected(U),
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

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct StripeUserInfo {
    pub customer_id: Option<String>,
    pub payment_methods: Vec<StripePaymentInfo>,
    pub subscriptions: Vec<StripeSubscriptionInfo>,
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
