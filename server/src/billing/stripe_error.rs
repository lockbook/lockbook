use tracing::*;

use super::stripe_model::{StripeDeclineCodeCatcher, StripeKnownDeclineCode};

#[derive(Debug)]
pub enum SimplifiedStripeError {
    CardDecline,
    InsufficientFunds,
    TryAgain,
    CardNotSupported,
    ExpiredCard,
    InvalidCardNumber,
    InvalidCardExpYear,
    InvalidCardExpMonth,
    InvalidCardCvc,
    Other(String),
}

impl From<stripe::StripeError> for SimplifiedStripeError {
    fn from(error: stripe::StripeError) -> Self {
        debug!(?error, "Stripe error");

        match error {
            stripe::StripeError::Stripe(stripe_error) => {
                simplify_stripe_error(stripe_error.code, stripe_error.decline_code)
            }
            _ => SimplifiedStripeError::Other(format!("Unexpected stripe error: {error:?}")),
        }
    }
}

fn simplify_stripe_error(
    error_code: Option<stripe::ErrorCode>, maybe_decline_code: Option<String>,
) -> SimplifiedStripeError {
    match error_code {
        None => SimplifiedStripeError::Other(format!(
            "Stripe error with no details: error_code: {error_code:?}, decline_code: {maybe_decline_code:?}"
        )),
        Some(error_code) => match error_code {
            stripe::ErrorCode::BalanceInsufficient => SimplifiedStripeError::InsufficientFunds,
            stripe::ErrorCode::CardDeclined => match maybe_decline_code {
                None => SimplifiedStripeError::CardDecline,
                Some(decline_code) => {
                    match serde_json::from_str::<StripeDeclineCodeCatcher>(&format!(
                        "\"{decline_code}\""
                    ))
                    .map_err(|e| {
                        SimplifiedStripeError::Other(format!(
                            "An error was encountered while serializing decline code: {e:?}"
                        ))
                    }) {
                        Ok(StripeDeclineCodeCatcher::Unknown(code)) => {
                            warn!(?code, "Unknown decline code from stripe");
                            SimplifiedStripeError::CardDecline
                        }
                        Ok(StripeDeclineCodeCatcher::Known(decline_code)) => match decline_code {
                            // Try again
                            StripeKnownDeclineCode::ApproveWithId
                            | StripeKnownDeclineCode::IssuerNotAvailable
                            | StripeKnownDeclineCode::ProcessingError
                            | StripeKnownDeclineCode::ReenterTransaction
                            | StripeKnownDeclineCode::TryAgainLater => {
                                SimplifiedStripeError::TryAgain
                            }

                            // Not supported
                            StripeKnownDeclineCode::CardNotSupported
                            | StripeKnownDeclineCode::CurrencyNotSupported => {
                                SimplifiedStripeError::CardNotSupported
                            }

                            // Balance or credit exceeded
                            StripeKnownDeclineCode::CardVelocityExceeded
                            | StripeKnownDeclineCode::InsufficientFunds
                            | StripeKnownDeclineCode::WithdrawalCountLimitExceeded => {
                                SimplifiedStripeError::InsufficientFunds
                            }

                            // Expired card
                            StripeKnownDeclineCode::ExpiredCard => {
                                SimplifiedStripeError::ExpiredCard
                            }

                            // Generic
                            StripeKnownDeclineCode::CallIssuer
                            | StripeKnownDeclineCode::DoNotTryAgain
                            | StripeKnownDeclineCode::DoNotHonor
                            | StripeKnownDeclineCode::NewAccountInformationAvailable
                            | StripeKnownDeclineCode::RestrictedCard
                            | StripeKnownDeclineCode::RevocationOfAllAuthorizations
                            | StripeKnownDeclineCode::RevocationOfAuthorization
                            | StripeKnownDeclineCode::SecurityViolation
                            | StripeKnownDeclineCode::ServiceNotAllowed
                            | StripeKnownDeclineCode::StopPaymentOrder
                            | StripeKnownDeclineCode::TransactionNotAllowed
                            | StripeKnownDeclineCode::Fraudulent
                            | StripeKnownDeclineCode::GenericDecline
                            | StripeKnownDeclineCode::LostCard
                            | StripeKnownDeclineCode::MerchantBlacklist
                            | StripeKnownDeclineCode::NoActionTaken
                            | StripeKnownDeclineCode::NotPermitted
                            | StripeKnownDeclineCode::PickupCard
                            | StripeKnownDeclineCode::StolenCard => {
                                SimplifiedStripeError::CardDecline
                            }

                            // Incorrect number
                            StripeKnownDeclineCode::IncorrectNumber
                            | StripeKnownDeclineCode::InvalidNumber => {
                                SimplifiedStripeError::InvalidCardNumber
                            }

                            // Incorrect cvc
                            StripeKnownDeclineCode::IncorrectCvc
                            | StripeKnownDeclineCode::InvalidCvc => {
                                SimplifiedStripeError::InvalidCardCvc
                            }

                            // Incorrect expiry month
                            StripeKnownDeclineCode::InvalidExpiryMonth => {
                                SimplifiedStripeError::InvalidCardExpMonth
                            }

                            // Incorrect expiry year
                            StripeKnownDeclineCode::InvalidExpiryYear => {
                                SimplifiedStripeError::InvalidCardExpYear
                            }
                        },
                        Err(e) => e,
                    }
                }
            },
            stripe::ErrorCode::ExpiredCard => SimplifiedStripeError::ExpiredCard,
            stripe::ErrorCode::InvalidCardType => SimplifiedStripeError::CardNotSupported,
            stripe::ErrorCode::InvalidCvc | stripe::ErrorCode::IncorrectCvc => {
                SimplifiedStripeError::InvalidCardCvc
            }
            stripe::ErrorCode::InvalidExpiryMonth => SimplifiedStripeError::InvalidCardExpMonth,
            stripe::ErrorCode::InvalidExpiryYear => SimplifiedStripeError::InvalidCardExpYear,
            stripe::ErrorCode::InvalidNumber | stripe::ErrorCode::IncorrectNumber => {
                SimplifiedStripeError::InvalidCardNumber
            }
            stripe::ErrorCode::ProcessingError => SimplifiedStripeError::TryAgain,
            _ => SimplifiedStripeError::Other(format!("Unexpected error code: {error_code:?}")),
        },
    }
}
