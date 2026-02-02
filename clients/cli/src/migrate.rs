use std::{
    mem,
    path::PathBuf,
    sync::atomic::{AtomicU16, Ordering},
};

use cli_rs::cli_error::CliResult;
use lb_rs::service::import_export::ImportStatus;
use tokio::fs;

use crate::{core, ensure_account_and_root};

use colored::Colorize;

#[tokio::main]
pub async fn bear(path: PathBuf) -> CliResult<()> {
    let lb = core().await?;
    ensure_account_and_root(&lb).await?;

    let mut entries = fs::read_dir(path).await?;
    let count = AtomicU16::new(0);

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_dir() {
            continue;
        };

        let file_name = path.file_name().unwrap().to_str().unwrap();

        if !file_name.ends_with(".md") {
            println!("Skipping {}, not a markdown file", file_name.yellow());
            continue;
        }

        let contents = fs::read_to_string(entry.path()).await.unwrap();
        let candidate_locations = path_from_tags(&contents);
        let selected_location = candidate_locations
            .iter()
            .max_by_key(|s| s.len())
            .cloned()
            .unwrap_or_else(|| "/".into());
        let image_path = PathBuf::from(
            entry
                .path()
                .to_str()
                .unwrap()
                .to_string()
                .strip_suffix(".md")
                .unwrap(),
        );

        println!("Importing {}", file_name.green());
        println!(
            "Destination {}, candidates: {:?}",
            selected_location.green(),
            candidate_locations
        );

        let parent = match lb.get_by_path(&selected_location).await {
            Ok(parent) => parent,
            Err(_) => lb.create_at_path(&format!("{selected_location}/")).await?,
        };

        let f = |status: ImportStatus| {
            if let ImportStatus::FinishedItem(_) = status {
                count.fetch_add(1, Ordering::Relaxed);
            }
        };

        lb.import_files(&[path], parent.id, &f).await?;

        if image_path.exists() {
            println!("Images from: {}", image_path.to_str().unwrap().green());
            lb.import_files(&[image_path], parent.id, &f).await?;
        } else {
            println!("no images found");
        }
        println!();
    }

    println!("{} files imported", count.load(Ordering::Relaxed).to_string().blue());
    Ok(())
}

fn path_from_tags(contents: &str) -> Vec<String> {
    let mut found_hash = false;
    let mut path = String::from("");
    let mut paths = vec![];

    for char in contents.chars() {
        if !found_hash {
            if char == '#' {
                found_hash = true;
                continue;
            } else {
                continue;
            }
        }

        if found_hash {
            if path.is_empty() && (char == ' ' || char == '#') {
                found_hash = false;
                continue;
            }

            if char.is_whitespace() || char == ',' || char == '.' {
                paths.push(mem::take(&mut path));
                found_hash = false;
                continue;
            }

            path.push(char);
        }
    }

    if !path.is_empty() {
        paths.push(path);
    }

    paths
}
