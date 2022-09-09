use crate::CliError;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use structopt::StructOpt;

use lockbook_core::{AdminDeleteAccountError, AdminDisappearFileError, Core, Error};
use lockbook_core::{AdminListPremiumUsersError, ShallowPaymentPlatform, Uuid};

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

    /// List all the premium users and their payment platform
    ListPremiumUsers,
}

pub fn admin(core: &Core, admin: Admin) -> Result<(), CliError> {
    match admin {
        Admin::DeleteAccount { username } => delete_account(core, username),
        Admin::DisappearFile { id } => disappear_file(core, id),
        Admin::ListPremiumUsers => list_premium_users(core),
    }
}

fn disappear_file(core: &Core, id: Uuid) -> Result<(), CliError> {
    let maybe_confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Are you sure you want to delete '{}'?", id))
        .interact_opt()?;

    if maybe_confirm.unwrap_or(false) {
        core.admin_disappear_file(id).map_err(|err| match err {
            Error::UiError(err) => match err {
                AdminDisappearFileError::InsufficientPermission => {
                    CliError::insufficient_permission()
                }
                AdminDisappearFileError::FileNotFound => CliError::file_id_not_found(id),
                AdminDisappearFileError::CouldNotReachServer => CliError::network_issue(),
                AdminDisappearFileError::ClientUpdateRequired => CliError::update_required(),
            },
            Error::Unexpected(msg) => CliError::unexpected(msg),
        })?;
    }
    Ok(())
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

fn list_premium_users(core: &Core) -> Result<(), CliError> {
    let premium_users = core.admin_list_premium_users().map_err(|err| match err {
        Error::UiError(err) => match err {
            AdminListPremiumUsersError::InsufficientPermission => {
                CliError::insufficient_permission()
            }
            AdminListPremiumUsersError::CouldNotReachServer => CliError::network_issue(),
            AdminListPremiumUsersError::ClientUpdateRequired => CliError::update_required(),
        },
        Error::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    if premium_users.is_empty() {
        println!("There are no premium users.");
    } else {
        for (user, platform) in premium_users {
            let platform_str = match platform {
                ShallowPaymentPlatform::GooglePlay => "Google Play",
                ShallowPaymentPlatform::Stripe => "Stripe",
            };

            println!("{}: {}", user, platform_str);
        }
    }

    Ok(())
}
