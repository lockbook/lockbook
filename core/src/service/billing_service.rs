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
            ApiError::Endpoint(SwitchAccountTierError::OldCardDoesNotExist) => {
                CoreError::OldCardDoesNotExist
            }
            ApiError::Endpoint(SwitchAccountTierError::NewTierIsOldTier) => {
                CoreError::NewTierIsOldTier
            }
            ApiError::Endpoint(SwitchAccountTierError::InvalidNumber) => {
                CoreError::InvalidCardNumber
            }
            ApiError::Endpoint(SwitchAccountTierError::InvalidExpYear) => {
                CoreError::InvalidCardExpYear
            }
            ApiError::Endpoint(SwitchAccountTierError::InvalidExpMonth) => {
                CoreError::InvalidCardExpMonth
            }
            ApiError::Endpoint(SwitchAccountTierError::InvalidCVC) => CoreError::InvalidCardCVC,
            ApiError::Endpoint(SwitchAccountTierError::CardDecline) => CoreError::CardDecline,
            ApiError::Endpoint(SwitchAccountTierError::InsufficientFunds) => {
                CoreError::CardHasInsufficientFunds
            }
            ApiError::Endpoint(SwitchAccountTierError::TryAgain) => CoreError::TryCardAgain,
            ApiError::Endpoint(SwitchAccountTierError::CardNotSupported) => {
                CoreError::CardNotSupported
            }
            ApiError::Endpoint(SwitchAccountTierError::ExpiredCard) => CoreError::ExpiredCard,
            ApiError::Endpoint(SwitchAccountTierError::CurrentUsageIsMoreThanNewTier) => {
                CoreError::CurrentUsageIsMoreThanNewTier
            }
            ApiError::Endpoint(SwitchAccountTierError::ConcurrentRequestsAreTooSoon) => {
                CoreError::ConcurrentRequestsAreTooSoon
            }
            ApiError::Endpoint(SwitchAccountTierError::UserNotFound) => {
                CoreError::AccountNonexistent
            }
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
