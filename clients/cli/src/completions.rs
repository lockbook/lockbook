use std::any::Any;

use clap::{ArgMatches, Command, Parser, Subcommand};
use clap_complete::Shell;
use lb::Core;

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

    let cli = Command::new("Built CLI");

    let cli = LbCli::augment_subcommands(cli);

    let matches = cli.get_matches_from(splitted);
//is there a way to get the type of an arg during the parsing process 

    matches.ids().for_each(|f| println!("{}", f.to_string()));

    let is_file_path = matches
        .subcommand()
        .unwrap()
        .1
        .ids()
        .any(|f| f.eq("target"));

    if is_file_path {
        list::list(
            core,
            ListArgs {
                recursive: false,
                long: false,
                paths: true,
                dirs: false,
                docs: false,
                ids: false,
                directory: matches
                    .subcommand()
                    .unwrap()
                    .1
                    .get_one::<String>("target")
                    .unwrap_or(&"/".to_string())
                    .to_string(),
            },
        )
        .ok()
        .unwrap_or(());
    }
    //for each arg that is a struct of type lbfile, output the completions.

    // if let Some(matches) = matches.get_("edit") {
    //     list::list(
    //         core,
    //         ListArgs {
    //             recursive: false,
    //             long: false,
    //             paths: true,
    //             dirs: false,
    //             docs: false,
    //             ids: false,
    //             directory: matches
    //                 .get_one::<String>("target")
    //                 .unwrap_or(&"/".to_string())
    //                 .to_string(),
    //         },
    //     )
    //     .ok()
    //     .unwrap_or(())
    // }
    //this matches the command, but i don't really care about that. i'm more concerned with the arg. one solution is to exist declarative paradigm in favor of imperative for increased control.
    // match LbCli::parse_from(splitted) {
    //     LbCli::Account(_) => todo!(),
    //     LbCli::Copy { disk_files, dest } => todo!(), // LbFolder
    //     LbCli::Debug(_) => todo!(),
    //     LbCli::Delete { target, force } => todo!(), // LbFile | LbFolder
    //     LbCli::Edit { target } => list::list(
    //         core,
    //         ListArgs {
    //             recursive: false,
    //             long: false,
    //             paths: true,
    //             dirs: false,
    //             docs: false,
    //             ids: false,
    //             directory: target,
    //         },
    //     )
    //     .ok()
    //     .unwrap_or(()), // LbFile
    //     LbCli::Export { target, dest } => todo!(),  // LbFile | LbFolder
    //     LbCli::List(_) => todo!(),                  //LbFile | LbFolder
    //     LbCli::Move { src_target, dest_target } => todo!(), // LbFile | LbFolder, LbFolder
    //     LbCli::New { path } => todo!(),             // LbNewFile | lbNewFolder
    //     LbCli::Print { target } => todo!(),         // LbFile
    //     LbCli::Rename { target, new_name } => todo!(), // LbFile | LbFolder
    //     LbCli::Share(_) => todo!(),                 // LbFile | LbFolder, PendingShareId
    //     LbCli::Sync => todo!(),
    //     LbCli::Completions { shell } => todo!(),
    //     LbCli::Complete { input, current } => todo!(),
    // }
    Ok(())
}
