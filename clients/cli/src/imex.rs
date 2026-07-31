use std::cell::Cell;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use cli_rs::cli_error::{CliError, CliResult};
use lb_rs::service::import_export::ImportStatus;

use crate::input::find_file;
use crate::{core, ensure_account_and_root};

#[tokio::main]
pub async fn import(disk: PathBuf, parent: String) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let parent = find_file(lb, &parent).await?.id;

    let total = Cell::new(0);
    let nth_file = Cell::new(0);
    let update_status = move |status: ImportStatus| match status {
        ImportStatus::CalculatedTotal(n_files) => total.set(n_files),
        ImportStatus::StartingItem(disk_path) => {
            nth_file.set(nth_file.get() + 1);
            print!("({}/{}) importing: {}... ", nth_file.get(), total.get(), disk_path);
            io::stdout().flush().unwrap();
        }
        ImportStatus::FinishedItem(_meta) => println!("done."),
    };

    lb.import_files(&[disk], parent, &update_status).await?;

    Ok(())
}

#[tokio::main]
pub async fn export(target: String, dest: PathBuf, force: bool, contents: bool) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let target_file = find_file(lb, &target).await?;

    println!("exporting '{}'...", target_file.name);

    // Document → explicit file path (dest is not an existing directory): write bytes directly.
    if target_file.is_document() && !dest_is_directory(&dest) {
        return export_document_to_path(lb, target_file.id, dest, force).await;
    }

    if !dest.exists() {
        fs::create_dir_all(&dest)?;
    } else if dest.is_file() {
        return Err(CliError::from(
            "destination exists as a file; use a directory as dest, or export a single document to that file path",
        ));
    }

    // Like rsync's trailing slash on the source (`src/ dest/`): place children in dest
    // rather than dest/<folder-name>/.
    if contents {
        if !target_file.is_folder() {
            return Err(CliError::from("--contents only applies when exporting a folder"));
        }
        let children = lb.get_children(&target_file.id).await?;
        if children.is_empty() {
            println!("folder is empty, nothing to export");
            return Ok(());
        }
        for child in children {
            lb.export_file(
                child.id,
                dest.clone(),
                force,
                &Some(|i| {
                    println!("{i:?}");
                }),
            )
            .await?;
        }
        return Ok(());
    }

    lb.export_file(
        target_file.id,
        dest,
        force,
        &Some(|i| {
            println!("{i:?}");
        }),
    )
    .await?;
    Ok(())
}

fn dest_is_directory(dest: &std::path::Path) -> bool {
    dest.is_dir()
        || dest
            .to_str()
            .map(|s| s.ends_with('/') || s.ends_with('\\'))
            .unwrap_or(false)
}

async fn export_document_to_path(
    lb: &lb_rs::Lb, id: lb_rs::Uuid, dest: PathBuf, force: bool,
) -> CliResult<()> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    if dest.exists() && !force {
        return Err(CliError::from(format!(
            "destination '{}' already exists (pass --force to overwrite)",
            dest.display()
        )));
    }

    let content = lb.read_document(id, true).await?;
    fs::write(&dest, content)?;
    println!("wrote {}", dest.display());
    Ok(())
}
