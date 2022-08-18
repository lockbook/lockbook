use crate::CliError;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use lockbook_core::{AdminDeleteAccountError, Core, Error};
use structopt::StructOpt;

#[derive(Debug, PartialEq, Eq, StructOpt)]
pub enum Admin {
    /// Delete a user
    DeleteAccount { username: String },
}

pub fn admin(core: &Core, admin: Admin) -> Result<(), CliError> {
    match admin {
        Admin::DeleteAccount { username } => delete_account(core, username),
    }
}

fn delete_account(core: &Core, username: String) -> Result<(), CliError> {
    let maybe_confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Are you sure you want to delete '{}'?", username))
        .interact_opt()?;

    if let Some(confirm) = maybe_confirm {
        if confirm {
            core.admin_delete_account(&username)
                .map_err(|err| match err {
                    Error::UiError(err) => match err {
                        AdminDeleteAccountError::InsufficientPermission => {
                            CliError::insufficient_permission()
                        }
                        AdminDeleteAccountError::UsernameNotFound => {
                            CliError::username_not_found(&username)
                        }
                        AdminDeleteAccountError::CouldNotReachServer => CliError::network_issue(),
                        AdminDeleteAccountError::ClientUpdateRequired => {
                            CliError::update_required()
                        }
                    },
                    Error::Unexpected(msg) => CliError::unexpected(msg),
                })?;

            println!("Account deleted!");
        }
    }

    Ok(())
}
