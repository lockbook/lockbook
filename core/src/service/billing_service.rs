use crate::service::api_service;
use crate::{account_repo, Config, CoreError};
use lockbook_models::api::{CreditCardInfo, GetRegisteredCreditCardsRequest, RegisterCreditCardRequest, RemoveCreditCardRequest};

pub fn add_credit_card(
    config: &Config,
    card_number: String,
    exp_month: String,
    exp_year: String,
    cvc: String,
) -> Result<String, CoreError> {
    let account = account_repo::get(config)?;

    api_service::request(
        &account,
        RegisterCreditCardRequest {
            card_number,
            exp_month,
            exp_year,
            cvc,
        },
    ).map_err(CoreError::from).map(|response| response.payment_method_id)
}

pub fn remove_credit_card(
    config: &Config,
    payment_method_id: String
) -> Result<(), CoreError> {
    let account = account_repo::get(config)?;

    api_service::request(
        &account,
        RemoveCreditCardRequest {
            payment_method_id
        },
    ).map_err(CoreError::from)
}

pub fn get_registered_credit_cards(
    config: &Config,
) -> Result<List<CreditCardInfo>, CoreError> {
    let account = account_repo::get(config)?;

    api_service::request(
        &account,
        GetRegisteredCreditCardsRequest {},
    ).map_err(CoreError::from).map(|response| response.credit_card_infos)
}
