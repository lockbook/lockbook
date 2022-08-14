use crate::CliError;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use lockbook_core::{AdminDeleteAccountError, Core, Error, FeatureFlag};
use structopt::StructOpt;

#[derive(Debug, PartialEq, StructOpt)]
pub enum Admin {
    /// Commands related to feature flags
    FeatureFlags(FeatureFlags),

    /// Delete a user
    DeleteAccount { username: String },
}

#[derive(Debug, PartialEq, StructOpt)]
pub enum FeatureFlags {
    /// Prints out the current state of all feature flags
    List,

    /// Manage new accounts feature flag
    NewAccounts {
        #[structopt(parse(try_from_str))]
        enable: bool,
    },
}

pub fn admin(core: &Core, admin: Admin) -> Result<(), CliError> {
    match admin {
        Admin::FeatureFlags(feature_flags) => features(core, feature_flags),
        Admin::DeleteAccount { username } => delete_account(core, username),
    }
}

fn features(core: &Core, features: FeatureFlags) -> Result<(), CliError> {
    match features {
        FeatureFlags::List => {
            let feature_flags = core.get_feature_flags_state()?;

            println!("New Accounts: {}", feature_flags.new_accounts);
        }
        FeatureFlags::NewAccounts { enable } => {
            features_switch_options(core, FeatureFlag::NewAccounts, enable)?
        }
    }

    Ok(())
}

fn features_switch_options(
    core: &Core, feature_flag: FeatureFlag, enable: bool,
) -> Result<(), CliError> {
    let feature_flag_str = match feature_flag {
        FeatureFlag::NewAccounts => "new accounts",
    };

    let maybe_confrim = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Are you sure you want to modify the '{}' feature flag to '{}'?",
            feature_flag_str, enable
        ))
        .interact_opt()?;

    if let Some(confirm) = maybe_confrim {
        if confirm {
            core.toggle_feature_flag(feature_flag, enable)?;

            println!("Feature flag toggled!");
        }
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
                            CliError::not_permissioned()
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
