use crate::{core, ensure_account, input};
use cli_rs::cli_error::CliResult;
use fs_extra::dir::CopyOptions;
use lb_fs::fs_impl::Drive;
use lb_rs::model::core_config::Config;

#[tokio::main]
pub async fn mount() -> CliResult<()> {
    let lb = &core().await?;
    ensure_account(lb).await?;
    warning()?;
    copy_data()?;
    Drive::mount().await?;
    Ok(())
}

fn warning() -> CliResult<()> {
    let answer: String = input::std_in(WARNING)?;
    if answer != "y" && answer != "Y" {
        return Err("Aborted.".into());
    }

    Ok(())
}

fn copy_data() -> CliResult<()> {
    let current_path = Config::writeable_path("cli");
    let target_path = format!("{}/.lockbook/drive", std::env::var("HOME").unwrap());

    fs_extra::copy_items(
        &[current_path],
        target_path,
        &CopyOptions::default().skip_exist(true).copy_inside(true),
    )
    .map_err(|err| format!("failed to copy cli -> drive: {err}"))?;

    Ok(())
}

const WARNING: &str = r#"lb-fs is in it's early stages, please expect bugs and report them. macOS is 8/10 stable,
linux is 7/10 stable, and windows is largely untested at the moment.

This version will cp your your CLI's data directory and create a dedicated one for lb-fs. Future
iterations will be more tightly integrated into host programs. lb-fs will sync changes to our server
on startup and then every 5 minutes.

This command will not return and print out logs from the NFS server. Once the server starts it will
mount a virtual file system to /tmp/lockbook. Ctrl-C'ing this process will shut down the server and
unmount the file system. For now, a clean umount is critical to not requiring a restart.

Press Y to proceed.
"#;
