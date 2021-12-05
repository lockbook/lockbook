use crate::service::api_service;
use crate::{account_repo, Config, CoreError};
use lockbook_models::api::RegisterCreditCard;

pub fn add_credit_card(
    config: &Config,
    card_number: String,
    exp_month: String,
    exp_year: String,
    cvc: String,
) -> Result<(), CoreError> {
    let account = account_repo::get(config)?;

    api_service::request(
        &account,
        RegisterCreditCard {
            card_number,
            exp_month,
            exp_year,
            cvc,
        },
    )?;

    Ok(())
}
