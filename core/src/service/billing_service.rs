use crate::model::errors::core_err_unexpected;
use crate::service::api_service;
use crate::service::api_service::ApiError;
use crate::{account_repo, Config, CoreError};
use lockbook_models::api::{
    AccountTier, GetCreditCardError, GetCreditCardRequest, SwitchAccountTierError,
    SwitchAccountTierRequest,
};

pub type CreditCardLast4Digits = String;

pub fn switch_account_tier(
    config: &Config, new_account_tier: AccountTier,
) -> Result<(), CoreError> {
    let account = account_repo::get(config)?;

    api_service::request(&account, SwitchAccountTierRequest { account_tier: new_account_tier })
        .map_err(|err| match err {
            ApiError::Endpoint(err) => match err {
                SwitchAccountTierError::OldCardDoesNotExist => CoreError::OldCardDoesNotExist,
                SwitchAccountTierError::NewTierIsOldTier => CoreError::NewTierIsOldTier,
                SwitchAccountTierError::InvalidCardNumber => CoreError::InvalidCardNumber,
                SwitchAccountTierError::InvalidCardExpYear => CoreError::InvalidCardExpYear,
                SwitchAccountTierError::InvalidCardExpMonth => CoreError::InvalidCardExpMonth,
                SwitchAccountTierError::InvalidCardCvc => CoreError::InvalidCardCvc,
                SwitchAccountTierError::CardDecline => CoreError::CardDecline,
                SwitchAccountTierError::InsufficientFunds => CoreError::CardHasInsufficientFunds,
                SwitchAccountTierError::TryAgain => CoreError::TryAgain,
                SwitchAccountTierError::CardNotSupported => CoreError::CardNotSupported,
                SwitchAccountTierError::ExpiredCard => CoreError::ExpiredCard,
                SwitchAccountTierError::CurrentUsageIsMoreThanNewTier => {
                    CoreError::CurrentUsageIsMoreThanNewTier
                }
                SwitchAccountTierError::ConcurrentRequestsAreTooSoon => {
                    CoreError::ConcurrentRequestsAreTooSoon
                }
                SwitchAccountTierError::UserNotFound => CoreError::AccountNonexistent,
            },
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            _ => core_err_unexpected(err),
        })?;

    Ok(())
}

pub fn get_credit_card(config: &Config) -> Result<CreditCardLast4Digits, CoreError> {
    let account = account_repo::get(config)?;

    Ok(api_service::request(&account, GetCreditCardRequest {})
        .map_err(|err| match err {
            ApiError::Endpoint(GetCreditCardError::NotAStripeCustomer) => {
                CoreError::NotAStripeCustomer
            }
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            _ => core_err_unexpected(err),
        })?
        .credit_card_last_4_digits)
}
