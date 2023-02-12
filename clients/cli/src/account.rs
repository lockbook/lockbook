use lb::Core;

use crate::input;
use crate::CliError;

#[derive(clap::Subcommand, Debug)]
pub enum AccountCmd {
    /// create a new lockbook account
    New {
        /// your desired username (will prompt if not provided)
        username: Option<String>,
        /// the server url to register with (will default first to API_URL then to the lockbook
        /// server)
        api_url: Option<String>,
    },
    /// import an existing account by piping in the account string
    Import,
    /// reveal your account's private key
    Export,
    /// start a monthly subscription for massively increased storage
    Subscribe,
    /// cancel an existing subscription
    Unsubscribe,
    /// show your account status
    Status,
}

pub fn account(core: &Core, cmd: AccountCmd) -> Result<(), CliError> {
    match cmd {
        AccountCmd::New { username, api_url } => new_acct(core, username, api_url),
        AccountCmd::Import => import_acct(core),
        AccountCmd::Export => export_acct(core),
        AccountCmd::Subscribe => subscribe(core),
        AccountCmd::Unsubscribe => unsubscribe(core),
        AccountCmd::Status => status(core),
    }
}

fn new_acct(
    core: &Core, maybe_username: Option<String>, maybe_api_url: Option<String>,
) -> Result<(), CliError> {
    let username = match maybe_username {
        Some(uname) => uname,
        None => {
            let mut uname: String = input("please enter a username: ")?;
            uname.retain(|c| c != '\n' && c != '\r');
            uname
        }
    };

    let api_url = maybe_api_url.unwrap_or_else(|| {
        std::env::var("API_URL").unwrap_or_else(|_| lb::DEFAULT_API_LOCATION.to_string())
    });

    println!("generating keys and checking for username availability...");
    core.create_account(&username, &api_url, true)?;

    println!("account created!");
    Ok(())
}

fn import_acct(core: &Core) -> Result<(), CliError> {
    if atty::is(atty::Stream::Stdin) {
        return Err(CliError::new("to import an existing lockbook account, pipe your account string into this command, e.g.:\npbpaste | lockbook account import"));
    }

    let mut account_string = String::new();
    std::io::stdin()
        .read_line(&mut account_string)
        .expect("failed to read from stdin");
    account_string.retain(|c| !c.is_whitespace());

    println!("importing account...");
    core.import_account(&account_string)?;

    println!("account imported! next, try to sync by running: lockbook sync");
    Ok(())
}

fn export_acct(core: &Core) -> Result<(), CliError> {
    let answer: String =
        input("your private key is about to be visible. do you want to proceed? [y/n]: ")?;
    if answer == "y" || answer == "Y" {
        println!("{}", core.export_account()?);
    }
    Ok(())
}

fn subscribe(core: &Core) -> Result<(), CliError> {
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
        let answer: String = input(format!("do you want use *{}? [y/n]: ", card))?;
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
            number: input("enter your card number: ")?,
            exp_year: input("expiration year: ")?,
            exp_month: input("expiration month: ")?,
            cvc: input("cvc: ")?,
        }
    };

    core.upgrade_account_stripe(lb::StripeAccountTier::Premium(payment_method))?;
    println!("subscribed!");
    Ok(())
}

fn unsubscribe(core: &Core) -> Result<(), CliError> {
    let answer: String = input("are you sure you would like to cancel your subscription? [y/n]: ")?;
    if answer == "y" || answer == "Y" {
        println!("cancelling subscription... ");
        core.cancel_subscription()?;
    }
    Ok(())
}

fn status(core: &Core) -> Result<(), CliError> {
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
