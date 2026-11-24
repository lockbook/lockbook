use std::io;
use std::str::FromStr;

use arboard::Clipboard;
use chrono::{TimeZone, Utc};
use cli_rs::cli_error::{CliError, CliResult};

use is_terminal::IsTerminal;
use lb_rs::DEFAULT_API_LOCATION;
use lb_rs::Lb;
use lb_rs::model::account::SUB_ACCOUNT_SEPARATOR;
use lb_rs::model::api::PaymentPlatform;

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
pub async fn import(api_url: ApiUrl) -> CliResult<()> {
    let lb = &core().await?;

    // if being piped, read from stdin
    if !io::stdin().is_terminal() {
        let mut account_string = String::new();
        io::stdin()
            .read_line(&mut account_string)
            .map_err(|e| CliError::from(format!("failed to read from stdin: {e}")))?;
        return import_key(lb, account_string.trim(), &api_url.0).await;
    }

    // try clipboard first
    if let Some(clip) = try_clipboard() {
        println!("trying clipboard...");
        if import_key(lb, &clip, &api_url.0).await.is_ok() {
            return Ok(());
        }
        println!("clipboard did not contain a valid key.");
    }

    // fall back to prompting
    let config = rpassword::ConfigBuilder::new()
        .password_feedback_mask('*')
        .build();
    let account_string =
        rpassword::prompt_password_with_config("paste your account key or phrase: ", config)
            .map_err(|e| CliError::from(format!("failed to read from stdin: {e}")))?;
    import_key(lb, account_string.trim(), &api_url.0).await
}

fn try_clipboard() -> Option<String> {
    Clipboard::new().ok()?.get_text().ok()
}

async fn import_key(lb: &Lb, key: &str, api_url: &str) -> CliResult<()> {
    println!("importing account...");
    lb.import_account(key, Some(api_url)).await?;
    println!("account imported! next, try to sync by running: lockbook sync");
    Ok(())
}

#[tokio::main]
pub async fn export(skip_check: bool, phrase: bool) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account(lb)?;

    // skip confirmation if piped or flag is set
    let interactive = io::stdout().is_terminal();
    let should_ask = interactive && !skip_check;

    if should_ask {
        // Keep "private key" in the compact-key warning — severity should stay sharp.
        let prompt = if phrase {
            "your account phrase is about to be visible. do you want to proceed? [y/n]: "
        } else {
            "your private key is about to be visible. do you want to proceed? [y/n]: "
        };
        let answer: String = input::std_in(prompt)?;
        if answer != "y" && answer != "Y" {
            return Ok(());
        }
    }

    if phrase {
        println!("{}", lb.export_account_phrase()?);
    } else {
        print!("{}", lb.export_account_private_key()?);
    }
    Ok(())
}

#[tokio::main]
pub async fn subscribe() -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account(lb)?;

    let url = lb.stripe_checkout_url().await?;
    println!("finish subscribing in your browser:\n{url}");
    Ok(())
}

#[tokio::main]
pub async fn billing() -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account(lb)?;

    let url = lb.stripe_billing_portal_url().await?;
    println!("manage your billing in your browser:\n{url}");
    Ok(())
}

#[tokio::main]
pub async fn unsubscribe() -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account(lb)?;

    let answer: String =
        input::std_in("are you sure you would like to cancel your subscription? [y/n]: ")?;
    if answer == "y" || answer == "Y" {
        println!("cancelling subscription... ");
        lb.cancel_subscription().await?;
        println!("canceled; premium lasts until the end of the period you paid for");
    }
    Ok(())
}

#[tokio::main]
pub async fn status() -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account(lb)?;

    println!("username: {}", lb.get_account()?.username);

    let last_synced = lb.get_last_synced_human().await?;
    println!("files last synced: {last_synced}");

    let unsynced = lb.local_changes().await;
    match unsynced.len() {
        0 => println!("unsynced changes: none"),
        1 => println!("unsynced changes: 1 file"),
        n => println!("unsynced changes: {n} files"),
    }

    let usage = lb.refresh_usage().await?;
    let pct = usage
        .data_cap
        .as_ref()
        .and_then(|cap| (usage.local.exact * 100).checked_div(cap.exact))
        .unwrap_or(0);

    if lb.get_account()?.username.contains(SUB_ACCOUNT_SEPARATOR) {
        println!("type: sub-account");
    } else if let Some(info) = lb.get_subscription_info().await? {
        let is_stripe = matches!(info.payment_platform, PaymentPlatform::Stripe { .. });
        match info.payment_platform {
            PaymentPlatform::Stripe { card_last_4_digits } => {
                if card_last_4_digits.is_empty() {
                    println!("type: Stripe");
                } else {
                    println!("type: Stripe, *{card_last_4_digits}");
                }
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
        let period_end_ms = if info.period_end >= 1_000_000_000_000 {
            info.period_end as i64
        } else {
            info.period_end as i64 * 1000
        };
        let renewal_date = Utc
            .timestamp_millis_opt(period_end_ms)
            .single()
            .map(|dt| dt.format("%B %d, %Y").to_string())
            .unwrap_or_else(|| info.period_end.to_string());
        if is_stripe {
            println!("paid through: {renewal_date}");
        } else {
            println!("renews on: {renewal_date}");
        }
    } else {
        println!("type: trial tier");
    }
    match &usage.data_cap {
        Some(cap) => {
            println!("usage: {} / {} ({}% utilized)", usage.local.readable, cap.readable, pct)
        }
        None => println!("usage: {}", usage.local.readable),
    }
    let mut unsynced = vec![];
    if usage.added.exact > 0 {
        unsynced.push(format!("+{}", usage.added.readable));
    }
    if usage.freed.exact > 0 {
        unsynced.push(format!("-{}", usage.freed.readable));
    }
    if !unsynced.is_empty() {
        println!("unsynced usage: {} (synced: {})", unsynced.join(" / "), usage.synced.readable);
    }
    if usage.over_cap() {
        println!("warning: over the data cap, sync will fail until space is freed");
    } else if pct >= 90 {
        println!("warning: storage is nearly full");
    }
    Ok(())
}

#[derive(Clone)]
pub struct ApiUrl(String);

impl Default for ApiUrl {
    fn default() -> Self {
        Self(std::env::var("API_URL").unwrap_or(DEFAULT_API_LOCATION.to_string()))
    }
}

impl FromStr for ApiUrl {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(String::from_str(s)?))
    }
}
