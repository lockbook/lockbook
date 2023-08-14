use std::{
    fmt::{Debug, Display},
    io::{self, Write},
    str::FromStr,
};

use cli_rs::cli_error::{CliError, CliResult};
use lb::{Core, File};

#[derive(Clone, Debug)]
pub enum FileInput {
    Id(lb::Uuid),
    Path(String),
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

pub fn file_completor(core: &Core, prompt: &str) -> CliResult<Vec<String>> {
    if prompt.starts_with(|c: char| c.is_ascii_hexdigit()) {
        return id_completor(core, prompt);
    }

    let working_dir = prompt
        .rfind('/')
        .map(|last| &prompt[..last + 1])
        .unwrap_or("");

    let parent = core.get_by_path(working_dir)?;

    let candidates = core
        .get_children(parent.id)?
        .into_iter()
        .map(|file| {
            let name = &file.name;
            if file.is_folder() {
                format!("{working_dir}{name}/")
            } else {
                format!("{working_dir}{name}")
            }
        })
        .collect();

    Ok(candidates)
}

pub fn id_completor(core: &Core, prompt: &str) -> CliResult<Vec<String>> {
    // todo: potential optimization opportunity inside core
    Ok(core
        .list_metadatas()?
        .into_iter()
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
