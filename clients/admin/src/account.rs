use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use basic_human_duration::ChronoHumanDuration;
use cli_rs::cli_error::{CliError, CliResult};
use dialoguer::Input;
use dialoguer::theme::ColorfulTheme;
use lb::blocking::Lb;
use lb::model::api::{
    AccountFilter, AccountIdentifier, AdminSetUserTierInfo, PaymentPlatformV2, UnixTimeMillis,
};
use libsecp256k1::PublicKey;
use time::{Duration, OffsetDateTime};

pub fn list(
    lb: &Lb, premium: bool, app_store_premium: bool, google_play_premium: bool,
    stripe_premium: bool,
) -> CliResult<()> {
    let filter = if premium {
        Some(AccountFilter::Premium)
    } else if app_store_premium {
        Some(AccountFilter::AppStorePremium)
    } else if google_play_premium {
        Some(AccountFilter::GooglePlayPremium)
    } else if stripe_premium {
        Some(AccountFilter::StripePremium)
    } else {
        None
    };

    let users = lb.admin_list_users(filter.clone())?;

    if users.is_empty() {
        let msg = match filter {
            None => "There are no users.",
            Some(AccountFilter::Premium) => "There are no premium users.",
            Some(AccountFilter::AppStorePremium) => "There are no premium app store users.",
            Some(AccountFilter::GooglePlayPremium) => "There are no premium google play users.",
            Some(AccountFilter::StripePremium) => "There are no premium stripe users.",
        };

        println!("{msg}");
    } else {
        for user in users {
            println!("{user}");
        }
    }

    Ok(())
}

pub fn info(lb: &Lb, username: String, public_key: String) -> CliResult<()> {
    let identifier = if !username.is_empty() {
        AccountIdentifier::Username(username)
    } else if !public_key.is_empty() {
        let key = base64::decode(public_key)?;
        let key = <&[u8; 33]>::try_from(key.as_slice())?;
        AccountIdentifier::PublicKey(PublicKey::parse_compressed(key)?)
    } else {
        return Err(CliError::from("pass --username=<name> or --public-key=<key>"));
    };

    let info = lb.admin_get_account_info(identifier)?;
    let payment_platform = match info.payment_platform {
        None => "none".to_string(),
        Some(PaymentPlatformV2::Stripe { card_last_4_digits }) => {
            format!("Stripe, card ending {card_last_4_digits}")
        }
        Some(PaymentPlatformV2::GooglePlay { account_state }) => {
            format!("Google Play, {account_state:?}")
        }
        Some(PaymentPlatformV2::AppStore { account_state }) => {
            format!("App Store, {account_state:?}")
        }
    };

    println!("username:\t\t{}", info.username);
    println!(
        "root:\t\t\t{}",
        info.root
            .map(|root| root.to_string())
            .unwrap_or("none".into())
    );
    println!("deleted:\t\t{}", if info.deleted { "yes" } else { "no" });
    println!("created:\t\t{}", when(info.created));
    println!("last seen:\t\t{}", when(info.last_seen));
    println!("payment platform:\t{payment_platform}");
    println!("usage:\t\t\t{}", info.usage);

    Ok(())
}

fn when(millis: Option<UnixTimeMillis>) -> String {
    let Some(millis) = millis else {
        return "unknown".into();
    };
    let Ok(at) = OffsetDateTime::from_unix_timestamp((millis / 1000) as i64) else {
        return format!("{millis} ms");
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let ago = Duration::milliseconds(now.saturating_sub(millis) as i64).format_human();

    format!(
        "{}-{:02}-{:02} {:02}:{:02} UTC ({ago})",
        at.year(),
        at.month() as u8,
        at.day(),
        at.hour(),
        at.minute()
    )
}

pub fn set_user_tier(lb: &Lb, username: String, info: AdminSetUserTierInfo) -> CliResult<()> {
    lb.admin_set_user_tier(&username, info)?;
    Ok(())
}

pub fn set_stripe_tier(lb: &Lb, username: String) -> CliResult<()> {
    let info = AdminSetUserTierInfo::Stripe {
        customer_id: ask("customer id")?,
        customer_name: ask("customer name")?,
        payment_method_id: ask("payment method id")?,
        last_4: ask("card's last 4 digits")?,
        subscription_id: ask("subscription id")?,
        expiration_time: ask("expiration time, in unix millis")?,
        account_state: ask("account state (Ok, Canceled or InvoiceFailed)")?,
    };

    set_user_tier(lb, username, info)
}

fn ask<T: FromStr>(prompt: &str) -> CliResult<T> {
    let answer: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact_text()?;

    answer
        .trim()
        .parse()
        .map_err(|_| CliError::from(format!("{answer} isn't a valid {prompt}")))
}
