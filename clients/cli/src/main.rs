mod account;
mod debug;
mod edit;
mod imex;
mod input;
mod lb_fs;
mod list;
mod migrate;
mod share;
mod stream;

use std::env;
use std::path::{Path, PathBuf};
use std::time::Instant;
use text_splitter::TextSplitter;
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
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Value;
use tokenizers::Tokenizer;

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
            Command::name("search").description("search document contents")
                .input(Arg::str("query"))
                .handler(|query| search(&query.get()))
        )
        .subcommand(
            Command::name("migrate-from").description("transfer files from an existing platform")
                .subcommand(
                    Command::name("bear").description("migrate your files from https://bear.app/ Export as md and using the 'export attachments' option.")
                        .input(Arg::<PathBuf>::name("disk-path").description("location of a bear export of files."))
                        .handler(|path| migrate::bear(path.get()))
                )
        )
        .subcommand(
            Command::name("sync").description("sync your local changes back to lockbook servers") // todo also back
                .handler(sync)
        )
        .with_completions()
        .parse()
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
    
    let model_dir = "models/all-MiniLM-L6-v2";
    let model_path = format!("{}/model.onnx", model_dir);
    let tokenizer_path = format!("{}/tokenizer.json", model_dir);
    
    if !Path::new(&model_path).exists() || !Path::new(&tokenizer_path).exists() {
        return Err(CliError::from("Model/Tokenizer not found in models/all-MiniLM-L6-v2"));
    }
    
    println!("Loading model...");
    
    let mut session = Session::builder()
        .map_err(|e| CliError::from(format!("Session error: {}", e)))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| CliError::from(format!("Session error: {}", e)))?
        .with_intra_threads(4)
        .map_err(|e| CliError::from(format!("Session error: {}", e)))?
        .commit_from_file(&model_path)
        .map_err(|e| CliError::from(format!("Load error: {}", e)))?;
    
    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| CliError::from(format!("Tokenizer error: {}", e)))?;
    
    // Create text splitter - chunks of ~200 tokens with 50 token overlap
    let splitter = TextSplitter::new(200);

    
    let create_embedding = |session: &mut Session, text: &str| -> Result<Vec<f32>, CliError> {
        let encoding = tokenizer.encode(text, true)
            .map_err(|e| CliError::from(format!("Tokenization failed: {}", e)))?;
        
        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let token_type_ids = encoding.get_type_ids();
        
        const MAX_LENGTH: usize = 256;
        
        let mut padded_ids = vec![0i64; MAX_LENGTH];
        let mut padded_mask = vec![0i64; MAX_LENGTH];
        let mut padded_types = vec![0i64; MAX_LENGTH];
        
        let len = input_ids.len().min(MAX_LENGTH);
        for i in 0..len {
            padded_ids[i] = input_ids[i] as i64;
            padded_mask[i] = attention_mask[i] as i64;
            padded_types[i] = token_type_ids[i] as i64;
        }
        
        let id_tensor = Value::from_array(([1, MAX_LENGTH], padded_ids))
            .map_err(|e| CliError::from(format!("Failed to create input tensor: {}", e)))?;
        let mask_tensor = Value::from_array(([1, MAX_LENGTH], padded_mask))
            .map_err(|e| CliError::from(format!("Failed to create mask tensor: {}", e)))?;
        let type_tensor = Value::from_array(([1, MAX_LENGTH], padded_types))
            .map_err(|e| CliError::from(format!("Failed to create type tensor: {}", e)))?;
        
        let outputs = session.run(ort::inputs![
            "input_ids" => id_tensor,
            "attention_mask" => mask_tensor,
            "token_type_ids" => type_tensor,
        ]).map_err(|e| CliError::from(format!("Model inference failed: {}", e)))?;
        
        let embeddings = outputs[0].try_extract_tensor::<f32>()
            .map_err(|e| CliError::from(format!("Failed to extract embeddings: {}", e)))?;
        
        let embedding_data = embeddings.1;
        
        const HIDDEN_SIZE: usize = 384;
        let mut pooled = vec![0.0f32; HIDDEN_SIZE];
        
        // Mean pooling
        for i in 0..len {
            for j in 0..HIDDEN_SIZE {
                pooled[j] += embedding_data[i * HIDDEN_SIZE + j];
            }
        }
        
        if len > 0 {
            for val in &mut pooled {
                *val /= len as f32;
            }
        }
        
        // L2 normalization
        let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut pooled {
                *val /= norm;
            }
        }
        
        Ok(pooled)
    };
    
    println!("Encoding query: \"{}\"", query);
    let query_embedding = create_embedding(&mut session, query)?;
    
    println!("Reading and chunking documents...");
    let files = lb.get_and_get_children_recursively(&lb.root().await?.id).await?;
    
    let mut results: Vec<(String, f32)> = Vec::new();
    let mut processed = 0;
    
    for file in files {
        if file.is_folder() {
            continue;
        }
        
        let content = match lb.read_document(file.id, false).await {
            Ok(c) => String::from_utf8_lossy(&c).to_string(),
            Err(_) => continue,
        };
        
        if content.trim().len() < 10 {
            continue;
        }
        
        // Split document into chunks
        let chunks: Vec<&str> = splitter.chunks(&content).collect();
        
        let mut best_similarity = 0.0f32;
        
        // Embed each chunk and find the best match
        for chunk in chunks {
            if chunk.trim().is_empty() {
                continue;
            }
            
            match create_embedding(&mut session, chunk) {
                Ok(chunk_embedding) => {
                    // Calculate cosine similarity
                    let similarity: f32 = query_embedding.iter()
                        .zip(chunk_embedding.iter())
                        .map(|(a, b)| a * b)
                        .sum();
                    
                    // Keep the best similarity score for this document
                    if similarity > best_similarity {
                        best_similarity = similarity;
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to encode chunk in '{}': {:?}", file.name, e);
                }
            }
        }
        
        if best_similarity > 0.0 {
            let path = lb.get_path_by_id(file.id).await
                .unwrap_or_else(|_| file.name.clone());
            
            results.push((path, best_similarity));
            processed += 1;
        }
    }
    
    println!("Processed {} documents", processed);
    
    // Sort by similarity (highest first)
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    // Display top 10 with colored scores
    println!("\nTop 10 Results:");
    println!("===============");
    
    if results.is_empty() {
        println!("No documents found.");
        return Ok(());
    }
    
    for (i, (path, score)) in results.iter().take(10).enumerate() {
        let score_str = format!("{:.4}", score);
        let colored_score = if *score > 0.7 {
            score_str.green()
        } else if *score > 0.5 {
            score_str.yellow()
        } else {
            score_str.red()
        };
        
        println!("{}. {} [{}]", i + 1, path, colored_score);
    }
    
    if results.len() > 10 {
        println!("\n{} more results not shown", results.len() - 10);
    }
    
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

fn ensure_account(lb: &Lb) -> CliResult<()> {
    if let Err(e) = lb.get_account() {
        if e.kind == LbErrKind::AccountNonexistent {
            return Err(CliError::from("no account found, run lockbook account import"));
        }
    }

    Ok(())
}

async fn ensure_account_and_root(lb: &Lb) -> CliResult<()> {
    ensure_account(lb)?;
    if let Err(e) = lb.root().await {
        if e.kind == LbErrKind::RootNonexistent {
            return Err(CliError::from("no root found, have you synced yet?"));
        }
    }

    Ok(())
}

#[tokio::main]
async fn sync() -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    lb.sync(None).await?;
    println!("Sync complete!");
    Ok(())
}