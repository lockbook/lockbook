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
use std::time::Instant;

use account::ApiUrl;
use cli_rs::arg::Arg;
use cli_rs::cli_error::{CliError, CliResult, Exit};
use cli_rs::command::Command;
use cli_rs::flag::Flag;
use cli_rs::parser::Cmd;

use colored::Colorize;
use input::FileInput;
use lb_rs::model::core_config::Config;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::path_ops::Filter;
use lb_rs::service::sync::SyncProgress;
use lb_rs::subscribers::search::{SearchConfig, SearchResult};
use lb_rs::{Lb, Uuid};

fn run() -> CliResult<()> {
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
                            account::new(username.get(), api_url.get())
                        })
                )
                .subcommand(
                    Command::name("import").description("import an existing account by piping in the account string")
                        .handler(account::import)
                )
                .subcommand(
                    Command::name("export").description("reveal your account's private key")
                        .input(Flag::bool("skip-check").description("don't ask for confirmation to reveal the private key"))
                        .handler(|skip_check| account::export(skip_check.get()))
                )
                .subcommand(
                    Command::name("subscribe").description("start a monthly subscription for massively increased storage")
                        .handler(account::subscribe)
                )
                .subcommand(
                    Command::name("unsubscribe").description("cancel an existing subscription")
                        .handler(account::unsubscribe)
                )
                .subcommand(
                    Command::name("status").description("show your account status")
                        .handler(account::status)
                )
        )
        .subcommand(
            Command::name("copy").description("import files from your file system into lockbook")
                .input(Arg::<PathBuf>::name("disk-path").description("path of file on disk"))
                .input(Arg::<FileInput>::name("dest")
                       .description("the path or id of a folder within lockbook to place the file.")
                       .completor(|prompt| input::file_completor(prompt, Some(Filter::FoldersOnly))))
                .handler(|disk, parent| imex::copy(disk.get(), parent.get()))
        )
        .subcommand(
            Command::name("debug").description("investigative commands")
                .subcommand(
                    Command::name("validate").description("helps find invalid states within your lockbook")
                        .handler(debug::validate)
                )
                .subcommand(
                    Command::name("info").description("print metadata associated with a file")
                        .input(Arg::<FileInput>::name("target").description("id or path of file to debug")
                            .completor(|prompt| input::file_completor(prompt, None)))
                        .handler(|target| debug::info(target.get()))
                )
                .subcommand(
                    Command::name("whoami").description("print who is logged into this lockbook")
                        .handler(debug::whoami)
                )
                .subcommand(
                    Command::name("whereami").description("print information about where this lockbook is stored and it's server url")
                        .handler(debug::whereami)
                )
                .subcommand(
                    Command::name("debuginfo").description("retrieve the debug-info string to help a lockbook engineer diagnose a problem")
                        .handler(debug::debug_info)
                )
        )
        .subcommand(
            Command::name("delete").description("delete a file")
                .input(Flag::bool("force"))
                .input(Arg::<FileInput>::name("target").description("path of id of file to delete")
                            .completor(|prompt| input::file_completor(prompt, None)))
                .handler(|force, target| delete(force.get(), target.get()))
        )
        .subcommand(
            Command::name("edit").description("edit a document")
                .input(edit::editor_flag())
                .input(Arg::<FileInput>::name("target").description("path or id of file to edit")
                            .completor(|prompt| input::file_completor(prompt, None)))
                .handler(|editor, target| edit::edit(editor.get(), target.get()))
        )
        .subcommand(
            Command::name("export").description("export a lockbook file to your file system")
                .input(Arg::<FileInput>::name("target")
                            .completor(|prompt| input::file_completor(prompt, None)))
                .input(Arg::<PathBuf>::name("dest"))
                .handler(|target, dest| imex::export(target.get(), dest.get()))
        )
        .subcommand(
            Command::name("fs")
                .description("use your lockbook files with your local filesystem by mounting an NFS drive to /tmp/lockbook")
                .handler(lb_fs::mount)
        )
        .subcommand(
            Command::name("list").description("list files and file information")
                .input(Flag::bool("long").description("'long listing format': displays id and sharee information in table format"))
                .input(Flag::bool("recursive").description("include all children of the given directory, recursively. Implicitly enables --paths"))
                .input(Flag::bool("paths").description("display the full path of any children"))
                .input(Arg::<FileInput>::name("target").description("file path location whose files will be listed")
                            .completor(|prompt| input::file_completor(prompt, Some(Filter::FoldersOnly)))
                            .default(FileInput::Path("/".to_string())))
                .handler(|long, recur, paths, target| list::list(long.get(), recur.get(), paths.get(), target.get()))
        )
        .subcommand(
            Command::name("move").description("move a file to a new parent")
                .input(Arg::<FileInput>::name("src-target").description("lockbook file path or ID of the file to move")
                            .completor(|prompt| input::file_completor(prompt, None)))
                .input(Arg::<FileInput>::name("dest").description("lockbook file path or ID of the new parent folder")
                            .completor(|prompt| input::file_completor(prompt, Some(Filter::FoldersOnly))))
                .handler(|src, dst| move_file(src.get(), dst.get()))
        )
        .subcommand(
            Command::name("new").description("create a new file at the given path or do nothing if it exists")
                .input(Arg::<FileInput>::name("path").description("create a new file at the given path or do nothing if it exists")
                            .completor(|prompt| input::file_completor(prompt, Some(Filter::FoldersOnly))))
                .handler(|target| create_file(target.get()))
        )
        .subcommand(
            Command::name("stream").description("interact with stdout and stdin")
                .subcommand(
                    Command::name("out")
                        .description("print a document to stdout")
                        .input(Arg::<FileInput>::name("target").description("lockbook file path or ID")
                            .completor(|prompt| input::file_completor(prompt, None)))
                        .handler(|target| stream::stdout(target.get()))
                )
                .subcommand(
                    Command::name("in")
                        .description("write stdin to a document")
                        .input(Arg::<FileInput>::name("target").description("lockbook file path or ID")
                            .completor(|prompt| input::file_completor(prompt, None)))
                        .input(Flag::bool("append").description("don't overwrite the specified lb file, append to it"))
                        .handler(|target, append| stream::stdin(target.get(), append.get()))
                )
        )
        .subcommand(
            Command::name("rename").description("rename a file")
                .input(Arg::<FileInput>::name("target").description("lockbook file path or ID of file to rename")
                            .completor(|prompt| input::file_completor(prompt, None)))
                .input(Arg::str("new_name"))
                .handler(|target, new_name| rename(target.get(), new_name.get()))
        )
        .subcommand(
            Command::name("share").description("sharing related commands")
                .subcommand(
                    Command::name("new").description("share a file with someone")
                        .input(Arg::<FileInput>::name("target").description("lockbook file path or ID of file to rename")
                            .completor(|prompt| input::file_completor(prompt, None)))
                        .input(Arg::str("username")
                            .completor(input::username_completor))
                        .input(Flag::bool("read-only"))
                        .handler(|target, username, ro| share::new(target.get(), username.get(), ro.get()))
                )
                .subcommand(
                    Command::name("pending").description("list pending shares")
                        .handler(share::pending)
                )
                .subcommand(
                    Command::name("accept").description("accept a pending share by adding it to your file tree")
                        .input(Arg::<Uuid>::name("pending-share-id").description("ID of pending share")
                                    .completor(share::pending_share_completor))
                        .input(Arg::<FileInput>::name("target").description("lockbook file path or ID of the folder you want to place this shared file")
                            .completor(|prompt| input::file_completor(prompt, Some(Filter::FoldersOnly))))
                        .handler(|id, dest| share::accept(&id.get(), dest.get()))
                )
                .subcommand(
                    Command::name("delete").description("delete a pending share")
                        .input(Arg::<Uuid>::name("share-id").description("ID of pending share to delete")
                               .completor(share::pending_share_completor))
                        .handler(|target| share::delete(target.get()))
                )
        )
        .subcommand(
            Command::name("search")
                .input(Arg::str("query"))
                .handler(|query| search(&query.get()))
        )
        .subcommand(
            Command::name("sync").description("sync your local changes back to lockbook servers") // todo also back
                .handler(sync)
        )
        .with_completions()
        .parse();

    Ok(())
}

fn main() {
    run().exit();
}

pub async fn core() -> CliResult<Lb> {
    Lb::init(Config::cli_config("cli"))
        .await
        .map_err(|err| CliError::from(err.to_string()))
}

#[tokio::main]
async fn search(query: &str) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let time = Instant::now();
    lb.build_index().await?;
    let build_time = time.elapsed();

    lb.get_search().await.tantivy_reader.reload().unwrap();

    let time = Instant::now();
    let results = lb.search(query, SearchConfig::PathsAndDocuments).await?;
    let search_time = time.elapsed();

    for result in results {
        match result {
            SearchResult::DocumentMatch { id: _, path, content_matches } => {
                println!("{}", format!("DOC: {path}").bold().blue());
                for content in content_matches {
                    let mut result = String::default();
                    for (i, c) in content.paragraph.char_indices() {
                        if content.matched_indices.contains(&i) {
                            result = format!("{result}{}", c.to_string().underline());
                        } else {
                            result = format!("{result}{c}");
                        }
                    }
                    println!("{result}");
                }
                println!();
            }
            SearchResult::PathMatch { id: _, path, matched_indices, score: _ } => {
                let mut result = String::default();
                for (i, c) in path.char_indices() {
                    if matched_indices.contains(&i) {
                        result = format!("{result}{}", c.to_string().underline());
                    } else {
                        result = format!("{result}{c}");
                    }
                }
                println!("{}", format!("PATH: {result}").bold().green());
                println!();
            }
        }
    }

    let build_time = format!("{build_time:?}").bold();
    let search_time = format!("{search_time:?}").bold();
    println!("Index built in {build_time}");
    println!("Search took {search_time}");

    Ok(())
}

#[tokio::main]
async fn sync() -> CliResult<()> {
    let lb = &core().await?;
    ensure_account(lb).await?;

    println!("syncing...");
    lb.sync(Some(Box::new(|sp: SyncProgress| {
        println!("{sp}");
    })))
    .await?;
    Ok(())
}

#[tokio::main]
async fn delete(force: bool, target: FileInput) -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let f = target.find(lb).await?;

    if !force {
        let mut phrase = format!("delete '{target}'");

        if f.is_folder() {
            let count = lb
                .get_and_get_children_recursively(&f.id)
                .await
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

    lb.delete(&f.id).await?;
    Ok(())
}

#[tokio::main]
async fn move_file(src: FileInput, dest: FileInput) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let src = src.find(lb).await?;
    let dest = dest.find(lb).await?;
    lb.move_file(&src.id, &dest.id).await?;
    Ok(())
}

#[tokio::main]
async fn create_file(path: FileInput) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let FileInput::Path(path) = path else {
        return Err(CliError::from("cannot create a file using ids"));
    };

    match lb.get_by_path(&path).await {
        Ok(_f) => Ok(()),
        Err(err) => match err.kind {
            LbErrKind::FileNonexistent => match lb.create_at_path(&path).await {
                Ok(_f) => Ok(()),
                Err(err) => Err(err.into()),
            },
            _ => Err(err.into()),
        },
    }
}

#[tokio::main]
async fn rename(target: FileInput, new_name: String) -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let id = target.find(lb).await?.id;
    lb.rename_file(&id, &new_name).await?;
    Ok(())
}

async fn ensure_account(lb: &Lb) -> CliResult<()> {
    if let Err(e) = lb.get_account().await {
        if e.kind == LbErrKind::AccountNonexistent {
            return Err(CliError::from("no account found, run lockbook account import"));
        }
    }

    Ok(())
}

async fn ensure_account_and_root(lb: &Lb) -> CliResult<()> {
    ensure_account(lb).await?;
    if let Err(e) = lb.root().await {
        if e.kind == LbErrKind::RootNonexistent {
            return Err(CliError::from("no root found, have you synced yet?"));
        }
    }

    Ok(())
}
