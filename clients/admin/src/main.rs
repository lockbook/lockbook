mod error;

use std::env;

use structopt::StructOpt;

use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;

use crate::error::Error;
use lockbook_core::{base64, Core};
use lockbook_core::{AccountFilter, AccountIdentifier, Config, PublicKey, Uuid};

#[derive(Debug, PartialEq, Eq, StructOpt)]
pub enum Admin {
    /// Delete a user
    DeleteAccount { username: String },

    /// Disappear a file
    ///
    /// When you delete a file you flip that file's is_deleted flag to false. In a disaster recovery
    /// scenario, you may want to *disappear* a file so that it never existed. This is useful in a
    /// scenario where your server let in an invalid file.
    DisappearFile { id: Uuid },

    /// Validates file trees of all users on the server and prints any failures
    ValidateAccount { username: String },

    /// List all users
    ListUsers {
        #[structopt(short, long)]
        premium: bool,

        #[structopt(short, long)]
        google_play_premium: bool,

        #[structopt(short, long)]
        stripe_premium: bool,
    },

    /// Get a user's info. This includes their username, public key, and payment platform.
    AccountInfo {
        #[structopt(short, long)]
        username: Option<String>,

        // A base 64 encoded and compressed public key
        #[structopt(short, long)]
        public_key: Option<String>,
    },
}

type Res<T> = Result<T, Error>;

pub fn main() {
    let writeable_path = match (env::var("LOCKBOOK_PATH"), env::var("HOME"), env::var("HOMEPATH")) {
        (Ok(s), _, _) => s,
        (Err(_), Ok(s), _) => format!("{}/.lockbook/cli", s),
        (Err(_), Err(_), Ok(s)) => format!("{}/.lockbook/cli", s),
        _ => panic!("no lockbook location"),
    };

    let core = Core::init(&Config { writeable_path, logs: true, colored_logs: true }).unwrap();

    let result = match Admin::from_args() {
        Admin::DeleteAccount { username } => delete_account(&core, username),
        Admin::DisappearFile { id } => disappear_file(&core, id),
        Admin::ValidateAccount { username } => server_validate(&core, username),
        Admin::ListUsers { premium, google_play_premium, stripe_premium } => {
            list_users(&core, premium, google_play_premium, stripe_premium)
        }
        Admin::AccountInfo { username, public_key } => account_info(&core, username, public_key),
    };

    result.unwrap_err();
}

fn disappear_file(core: &Core, id: Uuid) -> Res<()> {
    let maybe_confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Are you sure you want to delete '{}'?", id))
        .interact_opt()?;

    if maybe_confirm.unwrap_or(false) {
        core.admin_disappear_file(id)?;
    }
    Ok(())
}

fn delete_account(core: &Core, username: String) -> Res<()> {
    let maybe_confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Are you sure you want to delete '{}'?", username))
        .interact_opt()?;

    if maybe_confirm.unwrap_or(false) {
        core.admin_delete_account(&username)?;

        println!("Account deleted!");
    }

    Ok(())
}

fn list_users(
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

fn account_info(core: &Core, username: Option<String>, public_key: Option<String>) -> Res<()> {
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

fn server_validate(core: &Core, username: String) -> Res<()> {
    println!("Validating server...");

    let validation_failures = core.admin_server_validate(&username)?;
    for failure in validation_failures.tree_validation_failures {
        println!("tree validation failure: {:?}", failure);
    }
    for failure in validation_failures.documents_missing_content {
        println!("document missing content: {:?}", failure);
    }
    for failure in validation_failures.documents_missing_size {
        println!("document missing size: {:?}", failure);
    }

    Ok(())
}
