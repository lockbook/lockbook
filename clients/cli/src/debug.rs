use cli_rs::cli_error::{CliError, CliResult};
use lb_rs::service::debug::DebugInfoDisplay;

use crate::input::FileInput;
use crate::{core, ensure_account};

#[tokio::main]
pub async fn validate() -> CliResult<()> {
    let lb = core().await?;
    ensure_account(&lb)?;

    let warnings = lb
        .test_repo_integrity(true)
        .await
        .map_err(|err| CliError::from(format!("validating: {err:?}")))?;
    if warnings.is_empty() {
        return Ok(());
    }
    for w in &warnings {
        eprintln!("{w:#?}");
    }
    Err(CliError::from(format!("{} warnings found", warnings.len())))
}

#[tokio::main]
pub async fn info(target: FileInput) -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account(lb)?;

    let f = target.find(lb).await?;
    println!("{f:#?}");
    Ok(())
}

#[tokio::main]
pub async fn whoami() -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account(lb)?;

    println!("{}", lb.get_account()?.username);
    Ok(())
}

#[tokio::main]
pub async fn whereami() -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account(lb)?;

    let account = lb.get_account()?;
    let config = &lb.config;
    println!("Server: {}", account.api_url);
    println!("Core: {}", config.writeable_path);
    Ok(())
}

#[tokio::main]
pub async fn debug_info() -> Result<(), CliError> {
    let lb = &core().await?;
    println!(
        "{}",
        lb.debug_info("None Provided".to_string(), true)
            .await
            .to_string()
    );
    Ok(())
}
