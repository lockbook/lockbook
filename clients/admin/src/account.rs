use std::str::FromStr;

use lb::blocking::Lb;
use lb::model::api::{
    AccountFilter, AccountIdentifier, AdminSetUserTierInfo, AppStoreAccountState,
    GooglePlayAccountState, StripeAccountState,
};
use libsecp256k1::PublicKey;

use crate::error::Error;
use crate::{Res, SetUserTier};

pub fn list(
    lb: &Lb, premium: bool, app_store_premium: bool, google_play_premium: bool,
    stripe_premium: bool,
) -> Res<()> {
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

pub fn info(lb: &Lb, username: Option<String>, public_key: Option<String>) -> Res<()> {
    let identifier = if let Some(username) = username {
        AccountIdentifier::Username(username)
    } else if let Some(public_key) = public_key {
        AccountIdentifier::PublicKey(PublicKey::parse_compressed(<&[u8; 33]>::try_from(
            base64::decode(public_key)?.as_slice(),
        )?)?)
    } else {
        println!("Please specify a username or public key.");
        return Err(Error);
    };

    let account_info = lb.admin_get_account_info(identifier)?;
    println!("{account_info:#?}");

    Ok(())
}

pub fn set_user_tier(lb: &Lb, premium_info: SetUserTier) -> Res<()> {
    let (premium_info, username) = match premium_info {
        SetUserTier::Stripe {
            username,
            customer_id,
            customer_name,
            payment_method_id,
            last_4,
            subscription_id,
            expiration_time,
            account_state,
        } => (
            AdminSetUserTierInfo::Stripe {
                customer_id,
                customer_name,
                payment_method_id,
                last_4,
                subscription_id,
                expiration_time,
                account_state: StripeAccountState::from_str(&account_state)?,
            },
            username,
        ),
        SetUserTier::GooglePlay { username, purchase_token, expiration_time, account_state } => (
            AdminSetUserTierInfo::GooglePlay {
                purchase_token,
                expiration_time,
                account_state: GooglePlayAccountState::from_str(&account_state)?,
            },
            username,
        ),
        SetUserTier::AppStore {
            username,
            account_token,
            original_transaction_id,
            expiration_time,
            account_state,
        } => (
            AdminSetUserTierInfo::AppStore {
                account_token,
                original_transaction_id,
                expiration_time,
                account_state: AppStoreAccountState::from_str(&account_state)?,
            },
            username,
        ),
        SetUserTier::Free { username } => (AdminSetUserTierInfo::Free, username),
    };

    lb.admin_set_user_tier(&username, premium_info)?;

    Ok(())
}
