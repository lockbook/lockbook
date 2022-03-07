use serde::{Deserialize, Serialize};
use uuid::Uuid;

// This struct provides a fall back to unknown decline codes from Stripe. Since decline codes aren't parsed by "async-stripe" (a crate),
// we provide our own solution to parse them. Although, there are instances in which we may receive an unknown decline code. Rather than
// making serde handle this and return an internal error, we still handle the situation appropriately.
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum StripeDeclineCodeCatcher {
    Known(StripeKnownDeclineCode),
    Unknown(String),
}

// Decline codes for api version 2020-08-27
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

pub type Timestamp = u64;

// This is the stripe information for a single lockbook user. This is stored on redis.
#[derive(Serialize, Deserialize, Debug)]
pub struct StripeUserInfo {
    pub customer_id: Option<String>,
    pub customer_name: Uuid,
    pub payment_methods: Vec<StripePaymentInfo>,
    pub subscriptions: Vec<StripeSubscriptionInfo>,
    pub last_in_payment_flow: Timestamp,
}

impl Default for StripeUserInfo {
    fn default() -> Self {
        StripeUserInfo {
            customer_id: None,
            customer_name: Uuid::new_v4(),
            payment_methods: vec![],
            subscriptions: vec![],
            last_in_payment_flow: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StripePaymentInfo {
    pub id: String,
    pub last_4: String,
    pub created_at: Timestamp,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StripeSubscriptionInfo {
    pub id: String,
    pub period_end: Timestamp,
    pub is_active: bool,
}
