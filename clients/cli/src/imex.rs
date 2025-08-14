use std::{
    cell::Cell,
    fs,
    io::{self, Write},
    path::PathBuf,
};

use cli_rs::cli_error::CliResult;
use lb_rs::service::import_export::ImportStatus;

use crate::{core, ensure_account_and_root, input::FileInput};

#[tokio::main]
pub async fn copy(disk: PathBuf, parent: FileInput) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let parent = parent.find(lb).await?.id;

    let total = Cell::new(0);
    let nth_file = Cell::new(0);
    let _update_status = move |status: ImportStatus| match status {
        ImportStatus::CalculatedTotal(n_files) => total.set(n_files),
        ImportStatus::StartingItem(disk_path) => {
            nth_file.set(nth_file.get() + 1);
            print!("({}/{}) importing: {}... ", nth_file.get(), total.get(), disk_path);
            io::stdout().flush().unwrap();
        }
        ImportStatus::FinishedItem(_meta) => println!("done."),
    };

    lb.import_files(&[disk], parent, &None::<fn(ImportStatus)>).await?;

    Ok(())
}

#[tokio::main]
pub async fn export(target: FileInput, dest: PathBuf) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let target_file = target.find(lb).await?;

    println!("exporting '{}'...", target_file.name);
    if !dest.exists() {
        fs::create_dir(&dest)?;
    }

    // todo this is possibly ugly
    lb.export_file(
        target_file.id,
        dest,
        false,
        &Some(|i| {
            println!("{:?}", i);
        }),
    )
    .await?;
    Ok(())
}
