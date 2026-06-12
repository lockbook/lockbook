use std::fmt::{Debug, Display};
use std::io::{self, Write};
use std::str::FromStr;

use cli_rs::cli_error::{CliError, CliResult};
use lb_rs::model::file::File;
use lb_rs::model::path_ops::Filter;
use lb_rs::{Lb, Uuid};

use crate::core;

pub const ID_PREFIX_LEN: usize = 8;

fn looks_like_id(s: &str) -> bool {
    s.chars().all(|c| c == '-' || c.is_ascii_hexdigit())
}

pub async fn find_file(lb: &Lb, target: &str) -> CliResult<File> {
    let target = target.trim();

    if let Ok(id) = Uuid::from_str(target) {
        return Ok(lb.get_file_by_id(id).await?);
    }

    if target.len() >= ID_PREFIX_LEN && looks_like_id(target) {
        return find_by_id_prefix(lb, target).await;
    }

    Ok(lb.get_by_path(target).await?)
}

async fn find_by_id_prefix(lb: &Lb, prefix: &str) -> CliResult<File> {
    let mut found = None;
    for file in lb.list_metadatas().await? {
        if file.id.to_string().starts_with(prefix) {
            if found.is_some() {
                return Err(CliError::from(format!("id prefix '{prefix}' is ambiguous")));
            }
            found = Some(file);
        }
    }

    found.ok_or_else(|| CliError::from(format!("no file found with id prefix '{prefix}'")))
}

#[tokio::main]
pub async fn file_completor(prompt: &str, filter: Option<Filter>) -> CliResult<Vec<String>> {
    let lb = &core().await?;
    if !prompt.is_empty() && looks_like_id(prompt) {
        return id_completor(lb, prompt, filter).await;
    }

    let working_dir = prompt
        .rfind('/')
        .map(|last| &prompt[..last + 1])
        .unwrap_or("");

    let parent = lb.get_by_path(working_dir).await?;

    let candidates = lb
        .get_children(&parent.id)
        .await?
        .into_iter()
        .filter(|f| match filter {
            Some(Filter::FoldersOnly) => f.is_folder(),
            // documents could be inside folders
            // leaf nodes only doesn't make sense in this context
            _ => true,
        })
        .map(|file| {
            let name = &file.name;
            if file.is_folder() {
                format!("{working_dir}{name}/")
            } else {
                format!("{working_dir}{name}")
            }
        })
        .filter(|completion| completion.starts_with(prompt))
        .collect();

    Ok(candidates)
}

pub async fn id_completor(lb: &Lb, prompt: &str, filter: Option<Filter>) -> CliResult<Vec<String>> {
    // todo: potential optimization opportunity inside lb
    Ok(lb
        .list_metadatas()
        .await?
        .into_iter()
        .filter(|f| match filter {
            Some(Filter::FoldersOnly) => f.is_folder(),
            Some(Filter::DocumentsOnly) => f.is_document(),
            _ => true,
        })
        .map(|f| f.id.to_string())
        .filter(|cand| cand.starts_with(prompt))
        .collect())
}

#[tokio::main]
pub async fn username_completor(prompt: &str) -> CliResult<Vec<String>> {
    let lb = &core().await?;
    Ok(lb
        .known_usernames()
        .await?
        .into_iter()
        .filter(|cand| cand.starts_with(prompt))
        .collect())
}

pub fn std_in<T>(prompt: impl Display) -> Result<T, CliError>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
{
    print!("{prompt}");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .expect("failed to read from stdin");
    answer.retain(|c| c != '\n' && c != '\r');

    Ok(answer.parse::<T>().unwrap())
}
