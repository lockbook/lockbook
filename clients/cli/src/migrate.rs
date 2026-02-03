use std::{
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
    let notes_count = AtomicU16::new(0);

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_dir() {
            continue;
        };

        let file_name = path.file_name().unwrap().to_str().unwrap();

        if !file_name.ends_with(".md") {
            println!("Skipping {}, not a markdown file", file_name.yellow());
            println!();
            continue;
        }

        println!("Importing {}", file_name.green());

        let bytes = fs::read(entry.path()).await.unwrap();
        let contents = String::from_utf8_lossy(&bytes);

        if contents.contains('\u{FFFD}') {
            println!("{} contained invalid characters that were replaced", file_name.yellow());
        }
        let contents = contents.into_owned();

        let candidate_locations = candidate_locations_from_content(&contents);
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
            if let ImportStatus::FinishedItem(file) = status {
                if file.name.ends_with(".md") {
                    notes_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        };

        lb.import_files(&[path], parent.id, &f).await?;

        if image_path.exists() {
            println!("Images from: {}", image_path.to_str().unwrap().green());
            lb.import_files(&[image_path], parent.id, &f).await?;
        } else {
            println!("No images found");
        }
        println!();
    }

    println!("{} notes imported", notes_count.load(Ordering::Relaxed).to_string().blue());
    Ok(())
}

fn candidate_locations_from_content(contents: &str) -> Vec<String> {
    let mut prev_char: Option<char> = None;
    let mut in_tag = false;
    let mut path = String::from("");
    let mut paths = vec![];

    for char in contents.chars() {
        if !in_tag {
            if char == '#' {
                let is_valid_start = match prev_char {
                    None => true,
                    Some(c) => c.is_whitespace(),
                };

                if is_valid_start {
                    in_tag = true;
                    path.clear();
                }
            }
        } else if char.is_whitespace() || char == ',' || char == '.' {
            if !path.is_empty() {
                paths.push(path.clone());
            }
            in_tag = false;
        } else if char == '#' {
            // consecutive hashtags #foo#bar -> keep only foo. This mimics Bear's behavior.
            if !path.is_empty() {
                paths.push(path.clone());
            }
            path.clear();
            in_tag = false;
        } else {
            path.push(char);
        }

        prev_char = Some(char);
    }

    if in_tag && !path.is_empty() {
        paths.push(path);
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_candidate_locations_from_empty_content() {
        assert!(candidate_locations_from_content("").is_empty());
    }

    #[test]
    fn get_single_candidate_from_content() {
        let content = r#"# Meeting Notes

#meeting-notes"#;
        assert_eq!(candidate_locations_from_content(content), vec!["meeting-notes"]);
    }

    #[test]
    fn get_multiple_candidates_from_content() {
        let content = r#"# Meeting Notes

#meeting
#notes"#;
        assert_eq!(candidate_locations_from_content(content), vec!["meeting", "notes"]);
    }

    #[test]
    fn dont_get_candidate_locations_from_urls() {
        let content = r#"# Meeting Notes

http://url.com/#install
`http://url.com/#test`
[link](https://url.com/installing.html#mobile)
http://url.com/#install 
#notes"#;
        assert_eq!(candidate_locations_from_content(content), vec!["notes"]);
    }

    #[test]
    fn get_full_path_as_candidate_in_nested_hashtags() {
        let content = r#"# Meeting Notes
        #meeting/notes
        #work/team/project"#;
        assert_eq!(
            candidate_locations_from_content(content),
            vec!["meeting/notes", "work/team/project"]
        );
    }

    #[test]
    fn get_first_segment_as_candidate_from_consecutive_hashtags() {
        let content = "#meeting#notes";

        assert_eq!(candidate_locations_from_content(content), vec!["meeting"]);
    }
}
