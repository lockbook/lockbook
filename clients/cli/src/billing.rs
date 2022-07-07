use dialoguer::{Confirm, Input};

use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_core::GetSubscriptionInfoError;
use lockbook_core::GetUsageError;
use lockbook_core::PaymentMethod;
use lockbook_core::PaymentPlatform;
use lockbook_core::StripeAccountTier;
use lockbook_core::SubscriptionInfo;

use crate::{Billing, CliError};

pub fn billing(core: &Core, billing: Billing) -> Result<(), CliError> {
    match billing {
        Billing::Status => status(core),
        Billing::Subscribe => subscribe(core),
        Billing::UnSubscribe => cancel_subscription(core),
    }
}

fn status(core: &Core) -> Result<(), CliError> {
    let info = core.get_subscription_info()?;

    match info {
        Some(info) => {
            match info.payment_platform {
                PaymentPlatform::Stripe { card_last_4_digits } => {
                    println!("Type: Stripe, *{}", card_last_4_digits)
                }
                PaymentPlatform::GooglePlay { account_state } => {
                    println!("Type: Google Play");
                    println!("State: {:?}", account_state);
                }
            }

            println!("Renews on: {}", info.period_end);
        }
        None => {
            println!("Trial Tier")
        }
    }

    let cap = core.get_usage().map_err(|err| match err {
        LbError::UiError(GetUsageError::CouldNotReachServer) => CliError::network_issue(),
        LbError::UiError(GetUsageError::ClientUpdateRequired) => CliError::update_required(),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    println!(
        "Data Cap: {}, {}% utilized",
        cap.data_cap.readable,
        (cap.server_usage.exact * 100) / cap.data_cap.exact
    );

    Ok(())
}

fn subscribe(core: &Core) -> Result<(), CliError> {
    println!("Checking for existing payment methods...");
    let info = core.get_subscription_info()?;
    let existing_credit_card = info.and_then(|info| match info.payment_platform {
        PaymentPlatform::Stripe { card_last_4_digits } => Some(card_last_4_digits),
        PaymentPlatform::GooglePlay { .. } => None,
    });

    let payment_method = match existing_credit_card {
        Some(card) => {
            if reuse_old(&card)? {
                PaymentMethod::OldCard
            } else {
                solicit_card_info()?
            }
        }
        None => solicit_card_info()?,
    };

    use lockbook_core::UpgradeAccountStripeError::*;

    core.upgrade_account_stripe(StripeAccountTier::Premium(payment_method))
        .map_err(|err| match err {
            LbError::UiError(CouldNotReachServer) => CliError::network_issue(),
            LbError::UiError(OldCardDoesNotExist) => {
                CliError::unexpected("That card no longer exists!")
            }
            LbError::UiError(AlreadyPremium) => {
                CliError::unexpected("You're already subscribed to this tier!")
            }
            LbError::UiError(InvalidCardNumber) => CliError::billing("Invalid Card Number."),
            LbError::UiError(InvalidCardCvc) => CliError::billing("Invalid CVC."),
            LbError::UiError(InvalidCardExpYear) => CliError::billing("Invalid Expiration Year."),
            LbError::UiError(InvalidCardExpMonth) => CliError::billing("Invalid Expiration Month."),
            LbError::UiError(CardDecline) => CliError::billing("Card declined."),
            LbError::UiError(CardHasInsufficientFunds) => {
                CliError::billing("Card has insufficient funds.")
            }
            LbError::UiError(TryAgain) => CliError::billing("Try again later."),
            LbError::UiError(CardNotSupported) => {
                CliError::billing("Card not supported by stripe.")
            }
            LbError::UiError(ExpiredCard) => CliError::billing("Card expired."),
            LbError::UiError(ClientUpdateRequired) => CliError::update_required(),
            LbError::UiError(CurrentUsageIsMoreThanNewTier) => {
                CliError::billing("Your current usage exceeds the requested tier.")
            }
            LbError::UiError(ExistingRequestPending) => CliError::billing(
                "Another billing request is being processed, please wait and try again later.",
            ),
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })?;

    println!("Account upgrade successful!");
    Ok(())
}

fn cancel_subscription(core: &Core) -> Result<(), CliError> {
    println!("Cancelling subscription...");
    use lockbook_core::CancelSubscriptionError::*;
    core.cancel_subscription().map_err(|err| match err {
        LbError::UiError(NotPremium) => CliError::billing("You have no subscriptions to cancel!"),
        LbError::UiError(AlreadyCanceled) => {
            CliError::billing("This subscription has already been cancelled.")
        }
        LbError::UiError(UsageIsOverFreeTierDataCap) => CliError::billing(
            "Your usage exceeds the trial tier, please delete excess files first.",
        ),
        LbError::UiError(ExistingRequestPending) => CliError::billing(
            "Another billing request is being processed, please wait and try again later.",
        ),
        LbError::UiError(CouldNotReachServer) => CliError::network_issue(),
        LbError::UiError(ClientUpdateRequired) => CliError::update_required(),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    println!("Subscription cancelled successfully!");
    Ok(())
}

fn reuse_old(card: &str) -> Result<bool, CliError> {
    Ok(Confirm::new()
        .with_prompt(format!("Do you want use *{}?", card))
        .interact()?)
}

fn solicit_card_info() -> Result<PaymentMethod, CliError> {
    let number: String = Input::new()
        .with_prompt("Enter your card number")
        .interact_text()?;

    let exp_year: i32 = Input::new()
        .with_prompt("Expiration Year: ")
        .interact_text()?;

    let exp_month: i32 = Input::new()
        .with_prompt("Expiration Month: ")
        .interact_text()?;

    let cvc: String = Input::new().with_prompt("CVC: ").interact_text()?;

    Ok(PaymentMethod::NewCard { number, exp_year, exp_month, cvc })
}
