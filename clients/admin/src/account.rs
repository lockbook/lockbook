use crate::{Error, GooglePlayAccountState, Res, StripeAccountState, UpgradeToPremium};
use lockbook_core::{
    base64, AccountFilter, AccountIdentifier, AdminUpgradeToPremiumInfo, AppStoreAccountState,
    Core, PublicKey,
};
use std::str::FromStr;

pub fn list(
    core: &Core, premium: bool, google_play_premium: bool, stripe_premium: bool,
) -> Res<()> {
    let filter = if premium {
        Some(AccountFilter::Premium)
    } else if google_play_premium {
        Some(AccountFilter::GooglePlayPremium)
    } else if stripe_premium {
        Some(AccountFilter::StripePremium)
    } else {
        None
    };

    let users = core.admin_list_users(filter.clone())?;

    if users.is_empty() {
        let msg = match filter {
            None => "There are no users.",
            Some(AccountFilter::Premium) => "There are no premium users.",
            Some(AccountFilter::GooglePlayPremium) => "There are no premium google play users.",
            Some(AccountFilter::StripePremium) => "There are no premium stripe users.",
        };

        println!("{}", msg);
    } else {
        for user in users {
            println!("{}", user);
        }
    }

    Ok(())
}

pub fn info(core: &Core, username: Option<String>, public_key: Option<String>) -> Res<()> {
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

    let account_info = core.admin_get_account_info(identifier)?;
    println!("{:#?}", account_info);

    Ok(())
}

pub fn upgrade_to_premium(core: &Core, premium_info: UpgradeToPremium) -> Res<()> {
    let premium_info = match premium_info {
        UpgradeToPremium::Stripe {
            customer_id,
            customer_name,
            payment_method_id,
            last_4,
            subscription_id,
            expiration_time,
            account_state,
        } => AdminUpgradeToPremiumInfo::Stripe {
            customer_id,
            customer_name,
            payment_method_id,
            last_4,
            subscription_id,
            expiration_time,
            account_state: StripeAccountState::from_str(&account_state)?,
        },
        UpgradeToPremium::GooglePlay { purchase_token, expiration_time, account_state } => {
            AdminUpgradeToPremiumInfo::GooglePlay {
                purchase_token,
                expiration_time,
                account_state: GooglePlayAccountState::from_str(&account_state)?,
            }
        }
        UpgradeToPremium::AppStore {
            account_token,
            original_transaction_id,
            expiration_time,
            account_state,
        } => AdminUpgradeToPremiumInfo::AppStore {
            account_token,
            original_transaction_id,
            expiration_time,
            account_state: AppStoreAccountState::from_str(&account_state)?,
        },
    };

    core.admin_upgrade_to_premium(premium_info)?;

    Ok(())
}
