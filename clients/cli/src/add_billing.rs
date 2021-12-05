use crate::error::CliResult;
use crate::utils::{account, config, grab_line_stdin};
use lockbook_core::add_credit_card;
use std::io;

pub fn add_billing() -> CliResult<()> {
    account()?;

    let card_number = "grab_line_stdin(\"Enter your card number: \")?".to_string();
    let exp_month = "grab_line_stdin(\"Enter the expiration month [1-12]: \")?".to_string();
    let exp_year = "grab_line_stdin(\"Enter the expiration year (ex: 2025): \")?".to_string();
    let cvc = "grab_line_stdin(\"Enter the cvc: \")?".to_string();
    add_credit_card(&config()?, card_number, exp_month, exp_year, cvc).unwrap();
    Ok(())
}
