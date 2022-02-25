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
        .map_err(|e| match e {
            ApiError::Endpoint(SwitchAccountTierError::OldCardDoesNotExist) => {
                CoreError::OldCardDoesNotExist
            }
            ApiError::Endpoint(SwitchAccountTierError::NewTierIsOldTier) => {
                CoreError::NewTierIsOldTier
            }
            ApiError::Endpoint(SwitchAccountTierError::InvalidCreditCard(field)) => {
                CoreError::InvalidCreditCard(field)
            }
            ApiError::Endpoint(SwitchAccountTierError::CardDeclined(decline_type)) => {
                CoreError::CardDecline(decline_type)
            }
            ApiError::Endpoint(SwitchAccountTierError::CurrentUsageIsMoreThanNewTier) => {
                CoreError::CurrentUsageIsMoreThanNewTier
            }
            ApiError::Endpoint(SwitchAccountTierError::AlreadyInBillingWorkflow) => {
                CoreError::AlreadyInBillingWorkflow
            }
            ApiError::Endpoint(SwitchAccountTierError::UserNotFound) => {
                CoreError::AccountNonexistent
            }
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            _ => core_err_unexpected(e),
        })?;

    Ok(())
}

pub fn get_credit_card(config: &Config) -> Result<CreditCardLast4Digits, CoreError> {
    let account = account_repo::get(config)?;

    api_service::request(&account, GetCreditCardRequest {})
        .map_err(|e| match e {
            ApiError::Endpoint(GetCreditCardError::NotAStripeCustomer) => {
                CoreError::NotAStripeCustomer
            }
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            _ => core_err_unexpected(e),
        })
        .map(|response| response.credit_card_last_4_digits)
}
