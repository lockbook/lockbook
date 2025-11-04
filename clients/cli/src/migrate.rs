use std::{mem, path::PathBuf};

use cli_rs::cli_error::CliResult;
use tokio::fs;

use crate::{core, ensure_account_and_root};

#[tokio::main]
pub async fn bear(path: PathBuf) -> CliResult<()> {
    let lb = core().await?;
    ensure_account_and_root(&lb).await?;

    let mut entries = fs::read_dir(path).await?;

    while let Some(entry) = entries.next_entry().await? {
        if entry.path().is_dir() {
            continue;
        };

        let path = entry.path();
        let contents = fs::read_to_string(entry.path()).await.unwrap();
        let location = path_from_tags(&contents);

        println!("{path:?} -> {location:?}");
    }
    todo!()
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

    if paths.is_empty() {
        paths.push("/".into());
    }

    paths
}
