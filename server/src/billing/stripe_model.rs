use serde::{Deserialize, Serialize};

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
