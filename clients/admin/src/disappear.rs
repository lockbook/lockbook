use crate::{Res, Uuid};
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use lb::blocking::Lb;

pub fn file(lb: &Lb, id: Uuid) -> Res<()> {
    let maybe_confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Are you sure you want to delete '{id}'?"))
        .interact_opt()?;

    if maybe_confirm.unwrap_or(false) {
        lb.admin_disappear_file(id)?;

        println!("File disappeared");
    }
    Ok(())
}

pub fn account(lb: &Lb, username: String) -> Res<()> {
    let maybe_confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Are you sure you want to delete '{username}'?"))
        .interact_opt()?;

    if maybe_confirm.unwrap_or(false) {
        lb.admin_disappear_account(&username)?;

        println!("Account deleted!");
    }

    Ok(())
}
