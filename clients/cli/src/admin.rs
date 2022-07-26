use crate::{Admin, CliError, FeatureFlags, FeaturesSwitchOptions};
use lockbook_core::{
    AdminDeleteAccountError, Core, Error, FeatureFlag, GetFeatureFlagsStateError,
    ToggleFeatureFlagError,
};

pub fn admin(core: &Core, admin: Admin) -> Result<(), CliError> {
    match admin {
        Admin::FeatureFlags(feature_flags) => features(core, feature_flags),
        Admin::DeleteAccount { username } => delete_account(core, username),
    }
}

fn features(core: &Core, features: FeatureFlags) -> Result<(), CliError> {
    match features {
        FeatureFlags::List => {
            core.get_feature_flags_state().map_err(|err| match err {
                Error::UiError(err) => match err {
                    GetFeatureFlagsStateError::Unauthorized => CliError::unauthorized(),
                    GetFeatureFlagsStateError::CouldNotReachServer => CliError::network_issue(),
                    GetFeatureFlagsStateError::ClientUpdateRequired => CliError::update_required(),
                },
                Error::Unexpected(msg) => CliError::unexpected(msg),
            })?;
        }
        FeatureFlags::NewAccount(option) => {
            features_switch_options(core, FeatureFlag::NewAccounts, option)?
        }
    }

    Ok(())
}

fn features_switch_options(
    core: &Core, feature_flag: FeatureFlag, option: FeaturesSwitchOptions,
) -> Result<(), CliError> {
    let enable = match option {
        FeaturesSwitchOptions::SetOn => true,
        FeaturesSwitchOptions::SetOff => false,
    };

    core.toggle_feature_flag(feature_flag, enable)
        .map_err(|err| match err {
            Error::UiError(err) => match err {
                ToggleFeatureFlagError::Unauthorized => CliError::unauthorized(),
                ToggleFeatureFlagError::CouldNotReachServer => CliError::network_issue(),
                ToggleFeatureFlagError::ClientUpdateRequired => CliError::update_required(),
            },
            Error::Unexpected(msg) => CliError::unexpected(msg),
        })?;

    println!("Feature flag toggled!");

    Ok(())
}

fn delete_account(core: &Core, username: String) -> Result<(), CliError> {
    core.admin_delete_account(&username)
        .map_err(|err| match err {
            Error::UiError(err) => match err {
                AdminDeleteAccountError::Unauthorized => CliError::unauthorized(),
                AdminDeleteAccountError::UsernameNotFound => {
                    CliError::username_not_found(&username)
                }
                AdminDeleteAccountError::CouldNotReachServer => CliError::network_issue(),
                AdminDeleteAccountError::ClientUpdateRequired => CliError::update_required(),
            },
            Error::Unexpected(msg) => CliError::unexpected(msg),
        })?;

    println!("Account deleted!");

    Ok(())
}
