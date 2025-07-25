use std::fmt::{self, Debug, Display, Formatter};
use std::io::{self, Write};
use std::str::FromStr;

use cli_rs::cli_error::{CliError, CliResult};
use lb_rs::model::file::File;
use lb_rs::model::path_ops::Filter;
use lb_rs::{Lb, Uuid};

use crate::core;

#[derive(Clone, Debug)]
pub enum FileInput {
    Id(Uuid),
    Path(String),
}

impl Display for FileInput {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            FileInput::Id(inner) => write!(f, "{inner}"),
            FileInput::Path(inner) => write!(f, "{inner}"),
        }
    }
}

impl FileInput {
    pub async fn find(&self, lb: &Lb) -> CliResult<File> {
        let f = match self {
            FileInput::Id(id) => lb.get_file_by_id(*id).await?,
            FileInput::Path(path) => lb.get_by_path(path).await?,
        };

        Ok(f)
    }
}

impl FromStr for FileInput {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Uuid::from_str(s) {
            Ok(id) => Ok(FileInput::Id(id)),
            Err(_) => Ok(FileInput::Path(s.to_string())),
        }
    }
}

#[tokio::main]
pub async fn file_completor(prompt: &str, filter: Option<Filter>) -> CliResult<Vec<String>> {
    let lb = &core().await?;
    if !prompt.is_empty() && prompt.chars().all(|c| c == '-' || c.is_ascii_hexdigit()) {
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
