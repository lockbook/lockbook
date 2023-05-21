use std::io;
use std::io::Write;

use clap::{Command, CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Generator, Shell};
use lb::Core;
mod list;

const BASH_COMPLETIONS: &str = "
_x_lockbook_()
{
    _COMP_OUTPUTSTR=\"$( x-lockbook complete -- \"${COMP_WORDS[*]}\" ${COMP_CWORD} )\"
    if test $? -ne 0; then
        return 1
    fi
    COMPREPLY=($( echo -n \"$_COMP_OUTPUTSTR\" ))
}

complete -F _x_lockbook_ x-lockbook
";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
enum Cli {
    // If provided, outputs the completion file for given shell
    Completions { shell: Shell },

    Complete { input: String, current: i32 },

    Edit { target: String },
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, "lockbook-x", &mut io::stdout());
}
fn main() {
    let writeable_path = match (std::env::var("LOCKBOOK_PATH"), std::env::var("HOME")) {
        (Ok(s), _) => s,
        (Err(_), Ok(s)) => format!("{}/.lockbook/cli", s),
        _ => panic!(),
    };
    let core = Core::init(&lb::Config { writeable_path, logs: true, colored_logs: true }).unwrap();

    let cli = Cli::parse();

    match cli {
        Cli::Completions { shell } => match shell {
            Shell::Bash => print!("{}", BASH_COMPLETIONS),
            _ => {
                let mut cmd = Cli::command();
                print_completions(shell, &mut cmd);
            }
        },

        Cli::Complete { input, current } => {
            list::list(
                &core,
                list::ListArgs {
                    recursive: false,
                    long: false,
                    paths: true,
                    dirs: false,
                    docs: false,
                    ids: false,
                    directory: input.split(" ").last().unwrap_or("/").to_string(),
                },
            )
            .unwrap();
        }
        Cli::Edit { target } => {
            println!("hello from cli we have: {}", target);
        }
    }
}
