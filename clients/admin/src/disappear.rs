use crate::{Res, Uuid};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use lb::Core;

pub fn file(core: &Core, id: Uuid) -> Res<()> {
    let maybe_confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Are you sure you want to delete '{}'?", id))
        .interact_opt()?;

    if maybe_confirm.unwrap_or(false) {
        core.admin_disappear_file(id)?;

        println!("File disappeared");
    }
    Ok(())
}

pub fn account(core: &Core, username: String) -> Res<()> {
    let maybe_confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Are you sure you want to delete '{}'?", username))
        .interact_opt()?;

    if maybe_confirm.unwrap_or(false) {
        core.admin_disappear_account(&username)?;

        println!("Account deleted!");
    }

    Ok(())
}
