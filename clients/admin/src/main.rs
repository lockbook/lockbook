mod error;

use std::env;

use structopt::StructOpt;

use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;

use crate::error::Error;
use lockbook_core::{Config, Uuid};
use lockbook_core::{Core, PaymentPlatform};

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

    /// List all the premium users and their payment platform
    ListPremiumUsers {
        ///
        #[structopt(short, long)]
        google_play: bool,

        #[structopt(short, long)]
        stripe: bool,
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
        Admin::ListPremiumUsers { google_play, stripe } => {
            list_premium_users(&core, google_play, stripe)
        }
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

fn list_premium_users(core: &Core, google_play: bool, stripe: bool) -> Res<()> {
    let premium_users: Vec<(String, PaymentPlatform)> = core
        .admin_list_premium_users()?
        .into_iter()
        .filter(|(_, platform)| {
            if google_play {
                matches!(platform, PaymentPlatform::GooglePlay { .. })
            } else if stripe {
                matches!(platform, PaymentPlatform::Stripe { .. })
            } else {
                true
            }
        })
        .collect();

    if premium_users.is_empty() {
        if google_play {
            println!("There are no premium google play users.");
        } else if stripe {
            println!("There are no premium stripe users.");
        } else {
            println!("There are no premium users.");
        }
    } else {
        for (user, platform) in premium_users {
            let platform_str = match platform {
                PaymentPlatform::GooglePlay { .. } => "Google Play",
                PaymentPlatform::Stripe { .. } => "Stripe",
            };

            println!("{}: {}", user, platform_str);
        }
    }

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
