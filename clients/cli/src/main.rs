mod account;
mod debug;
mod input;

use account::ApiUrl;
use cli_rs::{
    arg::Arg,
    cli_error::{CliError, CliResult, Exit},
    command::Command,
    flag::Flag,
    parser::Cmd,
};

use input::FileInput;
use lb::Core;

fn run() -> CliResult<()> {
    let core = &core()?;

    Command::name("lockbook")
        .subcommand(
            Command::name("account")
                .description("account management commands")
                .subcommand(
                    Command::name("new")
                        .input(Arg::str("username").description("your desired username."))
                        .input(Flag::<ApiUrl>::new("api_url")
                            .description("location of the lockbook server you're trying to use. If not provided will check the API_URL env var, and then fall back to https://api.prod.lockbook.net"))
                        .handler(|username, api_url| {
                            account::new(core, username.get(), api_url.get())
                        })
                )
                .subcommand(
                    Command::name("import")
                        .description("import an existing account by piping in the account string")
                        .handler(|| account::import(core))
                )
                .subcommand(
                    Command::name("export")
                        .description("reveal your account's private key")
                        .input(Flag::bool("skip-check"))
                        .handler(|skip_check| account::export(core, skip_check.get()))
                )
                .subcommand(
                    Command::name("subscribe")
                        .description("start a monthly subscription for massively increased storage")
                        .handler(|| account::subscribe(core))
                )
                .subcommand(
                    Command::name("unsubscribe")
                        .description("cancel an existing subscription")
                        .handler(|| account::unsubscribe(core))
                )
                .subcommand(
                    Command::name("status")
                        .description("show your account status")
                        .handler(|| account::status(core))
                )
        )
        .subcommand(
            Command::name("copy")
        )
        .subcommand(
            Command::name("debug")
                .subcommand(
                    Command::name("validate")
                        .description("helps find invalid states within your lockbook")
                        .handler(|| todo!("validate"))
                )
                .subcommand(
                    Command::name("info")
                        .description("print metadata associated with a file")
                        .input(Arg::<FileInput>::name("target")
                            .completor(|prompt| input::file_completor(core, prompt).unwrap_or(vec![])))
                        .handler(|target| debug::info(core, target.get()))
                )
                .subcommand(
                    Command::name("whoami")
                        .description("print who is logged into this lockbook")
                        .handler(|| todo!("whoami"))
                )
                .subcommand(
                    Command::name("whereami")
                        .description("print information about where this lockbook is stored and it's server url")
                        .handler(|| todo!("whereami"))
                )
        )
        .subcommand(
            Command::name("delete")
                .description("delete a file")
                .input(Flag::bool("force"))
                .input(Arg::str("target"))
                .handler(|force, target| todo!("deleting: {}, {}", force.get(), target.get()))
        )
        .subcommand(
            Command::name("edit")
                .description("delete a file")
                .input(Arg::str("target"))
                .handler(|target| todo!("editing: {}", target.get()))
        )
        .subcommand(
            Command::name("export")
                .description("export a lockbook file to your file system")
                .input(Arg::str("target"))
                .input(Arg::str("dest"))
                .handler(|target, dest| todo!("exporting: {}, dest: {}", target.get(), dest.get()))
        )
        .subcommand(
            Command::name("list")
                .description("list files and file information")
                .input(Arg::str("target"))
                .handler(|target| todo!("listing: {}", target.get()))
        )
        .subcommand(
            Command::name("move")
                .description("move a file to a new parent")
                .input(Arg::str("src_target"))
                .input(Arg::str("dest_target"))
                .handler(|src, dst| todo!("moving: {} -> {}", src.get(), dst.get()))
        )
        .subcommand(
            Command::name("new")
                .description("create a new file at the given path or do nothing if it exists")
                .input(Arg::str("target"))
                .handler(|target| todo!("new: {}", target.get()))
        )
        .subcommand(
            Command::name("print")
                .description("print a document to stdout")
                .input(Arg::str("target"))
                .handler(|target| todo!("print: {}", target.get()))
        )
        .subcommand(
            Command::name("rename")
                .description("rename a file")
                .input(Arg::str("target"))
                .input(Arg::str("new_name"))
                .handler(|target, new_name| todo!("rename: {} -> {}", target.get(), new_name.get()))
        )
        .subcommand(
            Command::name("share")
                .description("sharing related commands")
                .subcommand(
                    Command::name("new")
                        .input(Arg::str("target"))
                        .input(Arg::str("username"))
                        .input(Flag::bool("read-only"))
                        .handler(|target, username, ro| todo!("new share {} {} {}", target.get(), username.get(), ro.get()))
                )
                .subcommand(
                    Command::name("pending")
                        .description("list pending shares")
                        .input(Flag::bool("full-ids")
                           .description("display full file ids instead of prefixes"))
                        .handler(|full_ids| todo!("pending {}", full_ids.get()))
                )
                .subcommand(
                    Command::name("accept")
                        .description("accept a pending share by adding it to your file tree")
                        .input(Arg::str("pending-share-id")
                               .description("ID of pending share"))
                        .input(Arg::str("dest")
                               .description("where you want the share to end up"))
                        .input(Arg::str("new_name").description("what you want to call this file"))
                        .handler(|id, dest, name| todo!("accept {} {} {}", id.get(), dest.get(), name.get()))
                )
                .subcommand(
                    Command::name("delete")
                        .description("delete a pending share")
                        .input(Arg::str("target")
                               .description("ID of pending share"))
                        .handler(|target| todo!("deleting share {}", target.get()))
                )
        )
        .subcommand(
            Command::name("sync")
                .description("sync your local changes back to lockbook servers")
                .handler(|| sync(core))
        )
        .with_completions()
        .parse();

    Ok(())
}

fn main() {
    run().exit();
}

fn core() -> CliResult<Core> {
    let writeable_path = match (std::env::var("LOCKBOOK_PATH"), std::env::var("HOME")) {
        (Ok(s), _) => s,
        (Err(_), Ok(s)) => format!("{}/.lockbook/cli", s),
        _ => return Err("no cli location".into()),
    };

    Core::init(&lb::Config { writeable_path, logs: true, colored_logs: true })
        .map_err(|err| CliError::from(err.msg))
}

fn sync(core: &Core) -> CliResult<()> {
    println!("syncing...");
    core.sync(Some(Box::new(|sp: lb::SyncProgress| {
        use lb::ClientWorkUnit::*;
        match sp.current_work_unit {
            PullMetadata => println!("pulling file tree updates"),
            PushMetadata => println!("pushing file tree updates"),
            PullDocument(f) => println!("pulling: {}", f.name),
            PushDocument(f) => println!("pushing: {}", f.name),
        };
    })))?;
    Ok(())
}
