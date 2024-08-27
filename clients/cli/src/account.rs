use std::{io, str::FromStr};

use cli_rs::cli_error::{CliError, CliResult};
use lb::{Core, WorkUnit};

use is_terminal::IsTerminal;

use crate::{ensure_account, input};

pub fn new(core: &Core, username: String, api_url: ApiUrl) -> CliResult<()> {
    println!("generating keys and checking for username availability...");
    core.create_account(&username, &api_url.0, true)?;
    println!("account created!");

    Ok(())
}

pub fn import(core: &Core) -> CliResult<()> {
    if io::stdin().is_terminal() {
        return Err(CliError::from("to import an existing lockbook account, pipe your account string into this command, e.g.:\npbpaste | lockbook account import".to_string()));
    }

    let mut account_string = String::new();
    io::stdin()
        .read_line(&mut account_string)
        .expect("failed to read from stdin");
    account_string.retain(|c| !c.is_whitespace());

    println!("importing account...");
    core.import_account(&account_string, None)?;

    println!("account imported! next, try to sync by running: lockbook sync");

    Ok(())
}

pub fn export(core: &Core, skip_check: bool) -> CliResult<()> {
    ensure_account(core)?;

    let should_ask = !skip_check;
    let mut should_show = false;

    if should_ask {
        let answer: String = input::std_in(
            "your private key is about to be visible. do you want to proceed? [y/n]: ",
        )?;
        if answer == "y" || answer == "Y" {
            should_show = true;
        }
    } else {
        should_show = true;
    }

    if should_show {
        println!("{}", core.export_account_private_key()?);
    }

    Ok(())
}

pub fn subscribe(core: &Core) -> Result<(), CliError> {
    ensure_account(core)?;

    println!("checking for existing payment methods...");
    let existing_card =
        core.get_subscription_info()?
            .and_then(|info| match info.payment_platform {
                lb::PaymentPlatform::Stripe { card_last_4_digits } => Some(card_last_4_digits),
                lb::PaymentPlatform::GooglePlay { .. } => None,
                lb::PaymentPlatform::AppStore { .. } => None,
            });

    let mut use_old_card = false;
    if let Some(card) = existing_card {
        let answer: String = input::std_in(format!("do you want use *{}? [y/n]: ", card))?;
        if answer == "y" || answer == "Y" {
            use_old_card = true;
        }
    } else {
        println!("no existing cards found...");
    }

    let payment_method = if use_old_card {
        lb::PaymentMethod::OldCard
    } else {
        lb::PaymentMethod::NewCard {
            number: input::std_in("enter your card number: ")?,
            exp_year: input::std_in("expiration year: ")?,
            exp_month: input::std_in("expiration month: ")?,
            cvc: input::std_in("cvc: ")?,
        }
    };

    core.upgrade_account_stripe(lb::StripeAccountTier::Premium(payment_method))?;
    println!("subscribed!");
    Ok(())
}

pub fn unsubscribe(core: &Core) -> Result<(), CliError> {
    ensure_account(core)?;

    let answer: String =
        input::std_in("are you sure you would like to cancel your subscription? [y/n]: ")?;
    if answer == "y" || answer == "Y" {
        println!("cancelling subscription... ");
        core.cancel_subscription()?;
    }
    Ok(())
}

pub fn status(core: &Core) -> Result<(), CliError> {
    ensure_account(core)?;

    let last_synced = core.get_last_synced_human_string()?;
    println!("files last synced: {last_synced}");

    let core_status = core.calculate_work()?;
    let local = core_status
        .work_units
        .iter()
        .filter_map(|wu| match wu {
            WorkUnit::LocalChange(id) => Some(id),
            WorkUnit::ServerChange(_) => None,
        })
        .count();
    let server = core_status
        .work_units
        .iter()
        .filter_map(|wu| match wu {
            WorkUnit::ServerChange(id) => Some(id),
            WorkUnit::LocalChange(_) => None,
        })
        .count();
    println!("files ready to push: {local}");
    println!("files ready to pull: {server}");

    let cap = core.get_usage()?;
    let pct = (cap.server_usage.exact * 100) / cap.data_cap.exact;

    if let Some(info) = core.get_subscription_info()? {
        match info.payment_platform {
            lb::PaymentPlatform::Stripe { card_last_4_digits } => {
                println!("type: Stripe, *{}", card_last_4_digits)
            }
            lb::PaymentPlatform::GooglePlay { account_state } => {
                println!("type: Google Play");
                println!("state: {:?}", account_state);
            }
            lb::PaymentPlatform::AppStore { account_state } => {
                println!("type: App Store");
                println!("state: {:?}", account_state);
            }
        }
        println!("renews on: {}", info.period_end);
    } else {
        println!("trial tier");
    }
    println!("data cap: {}, {}% utilized", cap.data_cap.readable, pct);
    Ok(())
}

#[derive(Clone)]
pub struct ApiUrl(String);

impl Default for ApiUrl {
    fn default() -> Self {
        Self(std::env::var("API_URL").unwrap_or("https://api.prod.lockbook.net".to_string()))
    }
}

impl FromStr for ApiUrl {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(String::from_str(s)?))
    }
}
