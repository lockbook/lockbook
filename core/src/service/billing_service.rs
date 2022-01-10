use crate::model::errors::core_err_unexpected;
use crate::service::api_service;
use crate::service::api_service::ApiError;
use crate::{account_repo, Config, CoreError};
use lockbook_models::api::{
    AccountTier, CreditCardInfo, GetLastRegisteredCreditCardRequest, SwitchAccountTierError,
    SwitchAccountTierRequest,
};

pub fn switch_account_tier(
    config: &Config,
    new_account_tier: AccountTier,
) -> Result<(), CoreError> {
    let account = account_repo::get(config)?;

    api_service::request(
        &account,
        SwitchAccountTierRequest {
            account_tier: new_account_tier,
        },
    )
    .map_err(|e| match e {
        ApiError::Endpoint(SwitchAccountTierError::PreexistingCardDoesNotExist) => {
            CoreError::PreexistingCardDoesNotExist
        }
        ApiError::Endpoint(SwitchAccountTierError::NewTierIsOldTier) => CoreError::NewTierIsOldTier,
        ApiError::Endpoint(SwitchAccountTierError::InvalidCreditCard(field)) => {
            CoreError::InvalidCreditCard(field)
        }
        ApiError::Endpoint(SwitchAccountTierError::CardDeclined(decline_type)) => {
            CoreError::CardDecline(decline_type)
        }
        ApiError::SendFailed(_) => CoreError::ServerUnreachable,
        _ => core_err_unexpected(e),
    })?;

    Ok(())
}

pub fn get_last_registered_card(config: &Config) -> Result<CreditCardInfo, CoreError> {
    let account = account_repo::get(config)?;

    api_service::request(&account, GetLastRegisteredCreditCardRequest {})
        .map_err(CoreError::from)
        .map(|response| response.credit_card)
}
