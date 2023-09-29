use std::{
    fmt::{self, Debug, Display, Formatter},
    io::{self, Write},
    str::FromStr,
};

use cli_rs::cli_error::{CliError, CliResult};
use lb::{Core, File, Filter};

#[derive(Clone, Debug)]
pub enum FileInput {
    Id(lb::Uuid),
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
    pub fn find(&self, core: &Core) -> CliResult<File> {
        let f = match self {
            FileInput::Id(id) => core.get_file_by_id(*id)?,
            FileInput::Path(path) => core.get_by_path(path)?,
        };

        Ok(f)
    }
}

impl FromStr for FileInput {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match lb::Uuid::from_str(s) {
            Ok(id) => Ok(FileInput::Id(id)),
            Err(_) => Ok(FileInput::Path(s.to_string())),
        }
    }
}

pub fn file_completor(core: &Core, prompt: &str, filter: Option<Filter>) -> CliResult<Vec<String>> {
    if !prompt.is_empty() && prompt.chars().all(|c| c == '-' || c.is_ascii_hexdigit()) {
        return id_completor(core, prompt, filter);
    }

    let working_dir = prompt
        .rfind('/')
        .map(|last| &prompt[..last + 1])
        .unwrap_or("");

    let parent = core.get_by_path(working_dir)?;

    let candidates = core
        .get_children(parent.id)?
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

pub fn id_completor(core: &Core, prompt: &str, filter: Option<Filter>) -> CliResult<Vec<String>> {
    // todo: potential optimization opportunity inside core
    Ok(core
        .list_metadatas()?
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

pub fn std_in<T>(prompt: impl Display) -> Result<T, CliError>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
{
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .expect("failed to read from stdin");
    answer.retain(|c| c != '\n' && c != '\r');

    Ok(answer.parse::<T>().unwrap())
}
