use std::io;

use cli_rs::cli_error::CliResult;
use is_terminal::IsTerminal;
use lb_rs::model::usage::bytes_to_human;

use crate::{core, ensure_account, input};

#[tokio::main]
pub async fn new(label: String) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account(lb)?;

    let username = lb.create_sub_account(&label).await?;
    println!("sub-account created: {username}");
    Ok(())
}

#[tokio::main]
pub async fn list() -> CliResult<()> {
    let lb = &core().await?;
    ensure_account(lb)?;

    let sub_accounts = lb.list_sub_accounts().await?;
    if sub_accounts.is_empty() {
        println!("no sub-accounts.");
        return Ok(());
    }
    for sub_account in sub_accounts {
        println!("{}\t{}", sub_account.username, bytes_to_human(sub_account.usage));
    }
    Ok(())
}

#[tokio::main]
pub async fn export(skip_check: bool, phrase: bool, username: String) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account(lb)?;

    if io::stdout().is_terminal() && !skip_check {
        let secret = if phrase { "account phrase" } else { "private key" };
        let answer: String = input::std_in(format!(
            "the {secret} of '{username}' is about to be visible. do you want to proceed? [y/n]: "
        ))?;
        if answer != "y" && answer != "Y" {
            return Ok(());
        }
    }

    if phrase {
        println!("{}", lb.export_sub_account_phrase(&username).await?);
    } else {
        print!("{}", lb.export_sub_account_private_key(&username).await?);
    }
    Ok(())
}

#[tokio::main]
pub async fn delete(force: bool, username: String) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account(lb)?;

    if !force {
        let answer: String = input::std_in(format!(
            "are you sure you want to delete '{username}' and the files it owns? [y/n]: "
        ))?;
        if answer != "y" && answer != "Y" {
            println!("aborted.");
            return Ok(());
        }
    }

    lb.delete_sub_account(&username).await?;
    Ok(())
}
