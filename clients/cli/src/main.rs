mod account;
mod debug;
mod edit;
mod imex;
mod input;
mod lb_fs;
mod list;
mod share;
mod stream;

use std::env;
use std::path::PathBuf;

use account::ApiUrl;
use cli_rs::{
    arg::Arg,
    cli_error::{CliError, CliResult, Exit},
    command::Command,
    flag::Flag,
    parser::Cmd,
};

use input::FileInput;

fn run() -> CliResult<()> {
    let core = &core()?;

    Command::name("lockbook")
        .description("The private, polished note-taking platform.") 
        .version(env!("CARGO_PKG_VERSION"))
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
                    Command::name("import").description("import an existing account by piping in the account string")
                        .handler(|| account::import(core))
                )
                .subcommand(
                    Command::name("export").description("reveal your account's private key")
                        .input(Flag::bool("skip-check").description("don't ask for confirmation to reveal the private key"))
                        .handler(|skip_check| account::export(core, skip_check.get()))
                )
                .subcommand(
                    Command::name("subscribe").description("start a monthly subscription for massively increased storage")
                        .handler(|| account::subscribe(core))
                )
                .subcommand(
                    Command::name("unsubscribe").description("cancel an existing subscription")
                        .handler(|| account::unsubscribe(core))
                )
                .subcommand(
                    Command::name("status").description("show your account status")
                        .handler(|| account::status(core))
                )
        )
        .subcommand(
            Command::name("copy").description("import files from your file system into lockbook")
                .input(Arg::<PathBuf>::name("disk-path").description("path of file on disk"))
                .input(Arg::<FileInput>::name("dest")
                       .description("the path or id of a folder within lockbook to place the file.")
                       .completor(|prompt| input::file_completor(core, prompt, Some(Filter::FoldersOnly))))
                .handler(|disk, parent| imex::copy(core, disk.get(), parent.get()))
        )
        .subcommand(
            Command::name("debug").description("investigative commands")
                .subcommand(
                    Command::name("validate").description("helps find invalid states within your lockbook")
                        .handler(|| debug::validate(core))
                )
                .subcommand(
                    Command::name("info").description("print metadata associated with a file")
                        .input(Arg::<FileInput>::name("target").description("id or path of file to debug")
                            .completor(|prompt| input::file_completor(core, prompt, None)))
                        .handler(|target| debug::info(core, target.get()))
                )
                .subcommand(
                    Command::name("whoami").description("print who is logged into this lockbook")
                        .handler(|| debug::whoami(core))
                )
                .subcommand(
                    Command::name("whereami").description("print information about where this lockbook is stored and it's server url")
                        .handler(|| debug::whereami(core))
                )
        )
        .subcommand(
            Command::name("delete").description("delete a file")
                .input(Flag::bool("force"))
                .input(Arg::<FileInput>::name("target").description("path of id of file to delete")
                            .completor(|prompt| input::file_completor(core, prompt, None)))
                .handler(|force, target| delete(core, force.get(), target.get()))
        )
        .subcommand(
            Command::name("edit").description("edit a document")
                .input(edit::editor_flag())
                .input(Arg::<FileInput>::name("target").description("path or id of file to edit")
                            .completor(|prompt| input::file_completor(core, prompt, None)))
                .handler(|editor, target| edit::edit(core, editor.get(), target.get()))
        )
        .subcommand(
            Command::name("export").description("export a lockbook file to your file system")
                .input(Arg::<FileInput>::name("target")
                            .completor(|prompt| input::file_completor(core, prompt, None)))
                .input(Arg::<PathBuf>::name("dest"))
                .handler(|target, dest| imex::export(core, target.get(), dest.get()))
        )
        .subcommand(
            Command::name("fs")
                .description("use your lockbook files with your local filesystem by mounting an NFS drive to /tmp/lockbook")
                .handler(|| lb_fs::mount(core))
        )
        .subcommand(
            Command::name("list").description("list files and file information")
                .input(Flag::bool("long").description("display more information"))
                .input(Flag::bool("recursive").description("include all children of the given directory, recursively"))
                .input(Flag::bool("paths").description("include more info (such as the file ID)"))
                .input(Arg::<FileInput>::name("target").description("file path location whose files will be listed")
                            .completor(|prompt| input::file_completor(core, prompt, Some(Filter::FoldersOnly)))
                            .default(FileInput::Path("/".to_string())))
                .handler(|long, recur, paths, target| list::list(core, long.get(), recur.get(), paths.get(), target.get()))
        )
        .subcommand(
            Command::name("move").description("move a file to a new parent")
                .input(Arg::<FileInput>::name("src-target").description("lockbook file path or ID of the file to move")
                            .completor(|prompt| input::file_completor(core, prompt, None)))
                .input(Arg::<FileInput>::name("dest").description("lockbook file path or ID of the new parent folder")
                            .completor(|prompt| input::file_completor(core, prompt, Some(Filter::FoldersOnly))))
                .handler(|src, dst| move_file(core, src.get(), dst.get()))
        )
        .subcommand(
            Command::name("new").description("create a new file at the given path or do nothing if it exists")
                .input(Arg::<FileInput>::name("path").description("create a new file at the given path or do nothing if it exists")
                            .completor(|prompt| input::file_completor(core, prompt, Some(Filter::FoldersOnly))))
                .handler(|target| create_file(core, target.get()))
        )
        .subcommand(
            Command::name("stream")
                .subcommand(
                    Command::name("out")
                        .description("print a document to stdout")
                        .input(Arg::<FileInput>::name("target").description("lockbook file path or ID")
                            .completor(|prompt| input::file_completor(core, prompt, None)))
                        .handler(|target| stream::stdout(core, target.get()))
                )
                .subcommand(
                    Command::name("in")
                        .description("write stdin to a document")
                        .input(Arg::<FileInput>::name("target").description("lockbook file path or ID")
                            .completor(|prompt| input::file_completor(core, prompt, None)))
                        .input(Flag::bool("append").description("don't overwrite the specified lb file, append to it"))
                        .handler(|target, append| stream::stdin(core, target.get(), append.get()))
                )
        )
        .subcommand(
            Command::name("rename").description("rename a file")
                .input(Arg::<FileInput>::name("target").description("lockbook file path or ID of file to rename")
                            .completor(|prompt| input::file_completor(core, prompt, None)))
                .input(Arg::str("new_name"))
                .handler(|target, new_name| rename(core, target.get(), new_name.get()))
        )
        .subcommand(
            Command::name("share").description("sharing related commands")
                .subcommand(
                    Command::name("new").description("share a file with someone")
                        .input(Arg::<FileInput>::name("target").description("lockbook file path or ID of file to rename")
                            .completor(|prompt| input::file_completor(core, prompt, None)))
                        .input(Arg::str("username"))
                        .input(Flag::bool("read-only"))
                        .handler(|target, username, ro| share::new(core, target.get(), username.get(), ro.get()))
                )
                .subcommand(
                    Command::name("pending").description("list pending shares")
                        .handler(|| share::pending(core))
                )
                .subcommand(
                    Command::name("accept").description("accept a pending share by adding it to your file tree")
                        .input(Arg::<Uuid>::name("pending-share-id").description("ID of pending share")
                                    .completor(|prompt| share::pending_share_completor(core, prompt)))
                        .input(Arg::<FileInput>::name("target").description("lockbook file path or ID of the folder you want to place this shared file")
                            .completor(|prompt| input::file_completor(core, prompt, Some(Filter::FoldersOnly))))
                        .handler(|id, dest| share::accept(core, id.get(), dest.get()))
                )
                .subcommand(
                    Command::name("delete").description("delete a pending share")
                        .input(Arg::<Uuid>::name("share-id").description("ID of pending share to delete")
                               .completor(|prompt| share::pending_share_completor(core, prompt)))
                        .handler(|target| share::delete(core, target.get()))
                )
        )
        .subcommand(
            Command::name("sync").description("sync your local changes back to lockbook servers") // todo also back
                .handler(|| sync(core))
        )
        .with_completions()
        .parse();

    Ok(())
}

fn main() {
    run().exit();
}

fn writable_path() -> CliResult<String> {
    let specified_path = env::var("LOCKBOOK_PATH");

    let default_path = env::var("HOME") // unix
        .or(env::var("HOMEPATH")) // windows
        .map(|home| format!("{home}/.lockbook/cli"));

    Ok(specified_path
        .or(default_path)
        .map_err(|_| "no cli location")?)
}

fn core() -> CliResult<Core> {
    let writeable_path = writable_path()?;

    Core::init(&lb::Config { writeable_path, logs: true, colored_logs: true })
        .map_err(|err| CliError::from(err.msg))
}

fn sync(core: &Core) -> CliResult<()> {
    ensure_account(core)?;

    println!("syncing...");
    core.sync(Some(Box::new(|sp: lb::SyncProgress| {
        println!("{sp}");
    })))?;
    Ok(())
}

fn delete(core: &Core, force: bool, target: FileInput) -> Result<(), CliError> {
    ensure_account_and_root(core)?;

    let f = target.find(core)?;

    if !force {
        let mut phrase = format!("delete '{target}'");

        if f.is_folder() {
            let count = core
                .get_and_get_children_recursively(f.id)
                .unwrap_or_default()
                .len() as u64
                - 1;
            match count {
                0 => {}
                1 => phrase = format!("{phrase} and its 1 child"),
                _ => phrase = format!("{phrase} and its {count} children"),
            };
        }

        let answer: String = input::std_in(format!("are you sure you want to {phrase}? [y/n]: "))?;
        if answer != "y" && answer != "Y" {
            println!("aborted.");
            return Ok(());
        }
    }

    core.delete_file(f.id)?;
    Ok(())
}

fn move_file(core: &Core, src: FileInput, dest: FileInput) -> CliResult<()> {
    ensure_account_and_root(core)?;

    let src = src.find(core)?;
    let dest = dest.find(core)?;
    core.move_file(src.id, dest.id)?;
    Ok(())
}

fn create_file(core: &Core, path: FileInput) -> CliResult<()> {
    ensure_account_and_root(core)?;

    let FileInput::Path(path) = path else {
        return Err(CliError::from("cannot create a file using ids"));
    };

    match core.get_by_path(&path) {
        Ok(_f) => Ok(()),
        Err(err) => match err.kind {
            CoreError::FileNonexistent => match core.create_at_path(&path) {
                Ok(_f) => Ok(()),
                Err(err) => Err(err.into()),
            },
            _ => Err(err.into()),
        },
    }
}

fn rename(core: &Core, target: FileInput, new_name: String) -> Result<(), CliError> {
    ensure_account_and_root(core)?;

    let id = target.find(core)?.id;
    core.rename_file(id, &new_name)?;
    Ok(())
}

fn ensure_account(core: &Core) -> CliResult<()> {
    if let Err(e) = core.get_account() {
        if e.kind == CoreError::AccountNonexistent {
            return Err(CliError::from("no account found, run lockbook account import"));
        }
    }

    Ok(())
}

fn ensure_account_and_root(core: &Core) -> CliResult<()> {
    ensure_account(core)?;
    if let Err(e) = core.get_root() {
        if e.kind == CoreError::RootNonexistent {
            return Err(CliError::from("no root found, have you synced yet?"));
        }
    }

    Ok(())
}
