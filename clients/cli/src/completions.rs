use std::str::FromStr;

use clap::{Command, CommandFactory, Subcommand};
use clap_complete::Shell;
use lb::Core;
use strum_macros::{AsRefStr, EnumString};

use crate::{
    error::CliError,
    list::{self, ListArgs},
    LbCli,
};

const BASH_COMPLETIONS: &str = "
_lockbook_complete_()
{
    _COMP_OUTPUTSTR=\"$( lockbook complete -- \"${COMP_WORDS[*]}\" ${COMP_CWORD} )\"
    if test $? -ne 0; then
        return 1
    fi
    COMPREPLY=($( echo -n \"$_COMP_OUTPUTSTR\" ))
}

complete -o nospace -F _lockbook_complete_ lockbook
";

#[derive(EnumString, AsRefStr)]
#[strum(serialize_all = "UPPERCASE")]
pub enum DynValueName {
    LbFilePath,
    LbAnyPath,
    LbFolderPath,
    PendingShareId,
}

pub fn generate_completions(shell: Shell) -> Result<(), CliError> {
    match shell {
        Shell::Bash | Shell::Zsh => print!("{}", BASH_COMPLETIONS),
        Shell::Fish => todo!(),
        _ => todo!(),
    }

    Ok(())
}

pub fn complete(core: &Core, input: String, current: i32) -> Result<(), CliError> {
    // todo: use shellwords instead https://crates.io/crates/shellwords
    let splitted = input.split_ascii_whitespace();
    // let selected_arg_value_name = splitted.nth(current as usize).unwrap();

    // manoeuver to switch from declarative to imperative pattern.
    let cli = Command::new("");
    let cli = LbCli::augment_subcommands(cli);
    let matches = cli.try_get_matches_from(splitted);

    if matches.is_err() {
        return Ok(());
    };
    let matches = matches.unwrap();

    let binding = LbCli::command();
    let selected_arg = binding
        .find_subcommand(matches.subcommand().unwrap().0)
        .unwrap()
        .get_arguments()
        .nth(current as usize - 2)
        .unwrap();

    let selected_arg_value_name = selected_arg.get_value_names().unwrap().get(0).unwrap();
    let selected_arg_value = matches
        .subcommand()
        .unwrap()
        .1
        .get_one::<String>(selected_arg.get_id().as_str())
        .unwrap_or(&"/".to_string())
        .to_string();

    match DynValueName::from_str(selected_arg_value_name).unwrap() {
        DynValueName::LbAnyPath => {
            list(core, selected_arg_value, false, false)?;
        }
        DynValueName::LbFolderPath => {
            list(core, selected_arg_value, true, false)?;
        }
        DynValueName::LbFilePath => {
            list(core, selected_arg_value, false, true)?;
        }
        DynValueName::PendingShareId => {
            todo!()
        }
    }

    Ok(())
}

fn list(core: &Core, input: String, dirs: bool, docs: bool) -> Result<(), CliError> {
    let mut tokens = input
        .split('/')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    let working_path = match tokens.len().cmp(&1) {
        std::cmp::Ordering::Less | std::cmp::Ordering::Equal => "/".to_string(),
        std::cmp::Ordering::Greater => {
            tokens.remove(tokens.len() - 1);
            tokens.join("/")
        }
    };

    println!("{}", working_path);
    list::list(
        core,
        ListArgs {
            recursive: false,
            long: false,
            paths: true,
            dirs,
            docs,
            ids: false,
            directory: working_path,
        },
    )?;

    Ok(())
}
