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

    match api_service::request(
        &account,
        SwitchAccountTierRequest { account_tier: new_account_tier },
    ) {
        Ok(_) => Ok(()),
        Err(ApiError::Endpoint(SwitchAccountTierError::OldCardDoesNotExist)) => {
            Err(CoreError::OldCardDoesNotExist)
        }
        Err(ApiError::Endpoint(SwitchAccountTierError::NewTierIsOldTier)) => {
            Err(CoreError::NewTierIsOldTier)
        }
        Err(ApiError::Endpoint(SwitchAccountTierError::InvalidCreditCard(field))) => {
            Err(CoreError::InvalidCreditCard(field))
        }
        Err(ApiError::Endpoint(SwitchAccountTierError::CardDeclined(decline_type))) => {
            Err(CoreError::CardDecline(decline_type))
        }
        Err(ApiError::Endpoint(SwitchAccountTierError::CurrentUsageIsMoreThanNewTier)) => {
            Err(CoreError::CurrentUsageIsMoreThanNewTier)
        }
        Err(ApiError::Endpoint(SwitchAccountTierError::ConcurrentRequestsAreTooSoon)) => {
            Err(CoreError::ConcurrentRequestsAreTooSoon)
        }
        Err(ApiError::Endpoint(SwitchAccountTierError::UserNotFound)) => {
            Err(CoreError::AccountNonexistent)
        }
        Err(ApiError::SendFailed(_)) => Err(CoreError::ServerUnreachable),
        Err(ApiError::ClientUpdateRequired) => Err(CoreError::ClientUpdateRequired),
        Err(e) => Err(core_err_unexpected(e)),
    }
}

pub fn get_credit_card(config: &Config) -> Result<CreditCardLast4Digits, CoreError> {
    let account = account_repo::get(config)?;

    match api_service::request(&account, GetCreditCardRequest {}) {
        Ok(resp) => Ok(resp.credit_card_last_4_digits),
        Err(ApiError::Endpoint(GetCreditCardError::NotAStripeCustomer)) => {
            Err(CoreError::NotAStripeCustomer)
        }
        Err(ApiError::SendFailed(_)) => Err(CoreError::ServerUnreachable),
        Err(ApiError::ClientUpdateRequired) => Err(CoreError::ClientUpdateRequired),
        Err(e) => Err(core_err_unexpected(e)),
    }
}
