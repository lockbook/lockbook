use std::io;
use std::str::FromStr;

use cli_rs::cli_error::{CliError, CliResult};

use is_terminal::IsTerminal;
use lb_rs::model::api::{PaymentMethod, PaymentPlatform, StripeAccountTier};
use lb_rs::model::work_unit::WorkUnit;

use crate::{core, ensure_account, input};

#[tokio::main]
pub async fn new(username: String, api_url: ApiUrl) -> CliResult<()> {
    let lb = core().await?;
    println!("generating keys and checking for username availability...");
    lb.create_account(&username, &api_url.0, true).await?;
    println!("account created!");

    Ok(())
}

#[tokio::main]
pub async fn import() -> CliResult<()> {
    let lb = &core().await?;
    if io::stdin().is_terminal() {
        return Err(CliError::from("to import an existing lockbook account, pipe your account string into this command, e.g.:\npbpaste | lockbook account import".to_string()));
    }

    let mut account_string = String::new();
    io::stdin()
        .read_line(&mut account_string)
        .expect("failed to read from stdin");
    account_string = account_string.trim().to_string();

    println!("importing account...");
    lb.import_account(&account_string, None).await?;

    println!("account imported! next, try to sync by running: lockbook sync");

    Ok(())
}

#[tokio::main]
pub async fn export(skip_check: bool) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account(lb).await?;

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
        println!("{}", lb.export_account_private_key().await?);
    }

    Ok(())
}

#[tokio::main]
pub async fn subscribe() -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account(lb).await?;

    println!("checking for existing payment methods...");
    let existing_card =
        lb.get_subscription_info()
            .await?
            .and_then(|info| match info.payment_platform {
                PaymentPlatform::Stripe { card_last_4_digits } => Some(card_last_4_digits),
                PaymentPlatform::GooglePlay { .. } => None,
                PaymentPlatform::AppStore { .. } => None,
            });

    let mut use_old_card = false;
    if let Some(card) = existing_card {
        let answer: String = input::std_in(format!("do you want use *{card}? [y/n]: "))?;
        if answer == "y" || answer == "Y" {
            use_old_card = true;
        }
    } else {
        println!("no existing cards found...");
    }

    let payment_method = if use_old_card {
        PaymentMethod::OldCard
    } else {
        PaymentMethod::NewCard {
            number: input::std_in("enter your card number: ")?,
            exp_year: input::std_in("expiration year: ")?,
            exp_month: input::std_in("expiration month: ")?,
            cvc: input::std_in("cvc: ")?,
        }
    };

    lb.upgrade_account_stripe(StripeAccountTier::Premium(payment_method))
        .await?;
    println!("subscribed!");
    Ok(())
}

#[tokio::main]
pub async fn unsubscribe() -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account(lb).await?;

    let answer: String =
        input::std_in("are you sure you would like to cancel your subscription? [y/n]: ")?;
    if answer == "y" || answer == "Y" {
        println!("cancelling subscription... ");
        lb.cancel_subscription().await?;
    }
    Ok(())
}

#[tokio::main]
pub async fn status() -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account(lb).await?;

    let last_synced = lb.get_last_synced_human().await?;
    println!("files last synced: {last_synced}");

    let lb_status = lb.calculate_work().await?;
    let local = lb_status
        .work_units
        .iter()
        .filter_map(|wu| match wu {
            WorkUnit::LocalChange(id) => Some(id),
            WorkUnit::ServerChange(_) => None,
        })
        .count();
    let server = lb_status
        .work_units
        .iter()
        .filter_map(|wu| match wu {
            WorkUnit::ServerChange(id) => Some(id),
            WorkUnit::LocalChange(_) => None,
        })
        .count();
    println!("files ready to push: {local}");
    println!("files ready to pull: {server}");

    let cap = lb.get_usage().await?;
    let pct = (cap.server_usage.exact * 100) / cap.data_cap.exact;

    if let Some(info) = lb.get_subscription_info().await? {
        match info.payment_platform {
            PaymentPlatform::Stripe { card_last_4_digits } => {
                println!("type: Stripe, *{card_last_4_digits}")
            }
            PaymentPlatform::GooglePlay { account_state } => {
                println!("type: Google Play");
                println!("state: {account_state:?}");
            }
            PaymentPlatform::AppStore { account_state } => {
                println!("type: App Store");
                println!("state: {account_state:?}");
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
