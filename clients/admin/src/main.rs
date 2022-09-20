mod error;

use std::env;

use structopt::StructOpt;

use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;

use crate::error::Error;
use lockbook_core::Core;
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
        google_play: bool,

        #[structopt(short, long)]
        stripe: bool,
    },

    /// Get a user's info. This includes their username, public key, and payment platform.
    AccountInfo {
        #[structopt(short, long)]
        username: Option<String>,

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
        Admin::ListUsers { premium, google_play, stripe } => {
            list_users(&core, premium, google_play, stripe)
        }
        Admin::AccountInfo { username, public_key } => user_info(&core, username, public_key),
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

fn list_users(core: &Core, premium: bool, google_play: bool, stripe: bool) -> Res<()> {
    let filter = if premium {
        Some(AccountFilter::Premium)
    } else if google_play {
        Some(AccountFilter::GooglePlayPremium)
    } else if stripe {
        Some(AccountFilter::StripePremium)
    } else {
        None
    };

    core.admin_list_users(filter)?
        .iter()
        .for_each(|user| println!("{}", user));

    Ok(())
}

fn user_info(core: &Core, username: Option<String>, public_key: Option<String>) -> Res<()> {
    let identifier = if let Some(username) = username {
        AccountIdentifier::Username(username)
    } else if let Some(public_key) = public_key {
        AccountIdentifier::PublicKey(PublicKey::parse_compressed(<&[u8; 33]>::try_from(
            public_key.as_bytes(),
        )?)?)
    } else {
        println!("Please specify a username or public key.");
        return Ok(());
    };

    let account_info = core.admin_get_account_info(identifier)?;

    println!("{:?}", account_info);

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
