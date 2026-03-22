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
use serde::{Deserialize, Serialize};
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
use std::cmp::min;
use ort::execution_providers::CUDAExecutionProvider;
use std::collections::{HashMap, HashSet};
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ChunkRecord {
    file_path: String,
    parent_chunk: String,
    child_text: String,
}



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
    let start_time = std::time::Instant::now();
    println!("{}", "🔍 Semantic Search".cyan().bold());
    println!("Query: {}\n", query);

    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    // ── Model paths ────────────────────────────────────────────────────────────
    let bi_model_path = "clients/models/e5-base-v2/model.onnx";
    let bi_tokenizer_path = "clients/models/e5-base-v2/tokenizer.json";

    // Check models exist
    for path in [bi_model_path, bi_tokenizer_path] {
        if !Path::new(path).exists() {
            return Err(CliError::from(format!("Model not found: {}", path)));
        }
    }

    // ── Index paths ────────────────────────────────────────────────────────────
    let index_dir = Path::new("search_index");
    let vectors_path = index_dir.join("vectors.bin");
    let chunks_path = index_dir.join("chunks.json");
    let manifest_path = index_dir.join("manifest.json");

    // ── Config for E5 ─────────────────────────────────────────────────────────
    const BI_HIDDEN: usize = 768;      // E5 uses 768
    const BI_MAX_LEN: usize = 128;
    const CHILD_CHARS: usize = 512;    // Optimal chunk size
    const PARENT_CHARS: usize = 2048;  // Parent context
    const CHILD_OVERLAP: usize = 51;
    const PARENT_OVERLAP: usize = 205;
    const TOP_K: usize = 5;            // Number of results to show
    const MAX_CHUNKS_PER_FILE: usize = 5000;

    // ── Helpers ────────────────────────────────────────────────────────────────

    // Unicode-safe char-based chunking
    let chunk_text = |text: &str, size: usize, overlap: usize| -> Vec<String> {
        let chars: Vec<char> = text.trim().chars().collect();
        let total = chars.len();
        if total == 0 {
            return vec![];
        }
        if total <= size {
            return vec![chars.iter().collect()];
        }

        let mut chunks = Vec::new();
        let mut start = 0usize;
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 10000;

        while start < total && chunks.len() < MAX_CHUNKS_PER_FILE {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                break;
            }
            
            let mut end = std::cmp::min(start + size, total);

            // try to break on ". " within last 20%
            let search_from = end.saturating_sub(size / 5);
            if let Some(break_pos) = (search_from..end.saturating_sub(1))
                .rev()
                .find(|&i| chars[i] == '.' && chars[i + 1] == ' ')
                .map(|i| i + 1)
            {
                end = break_pos;
            }

            let chunk: String = chars[start..end].iter().collect();
            let chunk = chunk.trim().to_string();
            if !chunk.is_empty() {
                chunks.push(chunk);
            }
            if end >= total {
                break;
            }

            let next = end.saturating_sub(overlap);
            let old_start = start;
            start = if next <= start { end } else { next };
            
            if start == old_start && start < total {
                start = end;
            }
        }
        
        chunks
    };

    // Simple hash of file content for change detection
    let hash_content = |content: &str| -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        content.hash(&mut h);
        h.finish()
    };

    // ── Step 1: Scan files and detect changes ──────────────────────────────────
    println!("📁 Scanning files...");
    let files = lb
        .get_and_get_children_recursively(&lb.root().await?.id)
        .await?;

    let mut manifest: HashMap<String, u64> = if manifest_path.exists() {
        let raw = std::fs::read_to_string(&manifest_path).unwrap_or_else(|_| "{}".to_string());
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // Collect all .txt and .md files
    let mut current_files: Vec<(String, String)> = Vec::new();
    for file in &files {
        if file.is_folder() {
            continue;
        }
        let name = file.name.to_lowercase();
        if !name.ends_with(".txt") && !name.ends_with(".md") {
            continue;
        }

        let content = match lb.read_document(file.id, false).await {
            Ok(c) => String::from_utf8_lossy(&c).to_string(),
            Err(_) => continue,
        };
        if content.trim().len() < 20 {
            continue;
        }

        let path = lb
            .get_path_by_id(file.id)
            .await
            .unwrap_or_else(|_| file.name.clone());

        current_files.push((path, content));
    }

    if current_files.is_empty() {
        println!("No documents found. Add some .txt or .md files first.");
        return Ok(());
    }

    // Check for changes
    let needs_reindex: Vec<&(String, String)> = current_files
        .iter()
        .filter(|(path, content)| {
            let current_hash = hash_content(content);
            manifest.get(path) != Some(&current_hash)
        })
        .collect();

    let current_paths: HashSet<&str> = current_files.iter().map(|(p, _)| p.as_str()).collect();
    let has_deletions = manifest.keys().any(|p| !current_paths.contains(p.as_str()));

    let index_exists = vectors_path.exists() && chunks_path.exists();
    let index_is_valid = if index_exists {
        std::fs::read_to_string(&chunks_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Vec<serde_json::Value>>(&raw).ok())
            .map(|chunks| !chunks.is_empty())
            .unwrap_or(false)
    } else {
        false
    };
    
    let needs_full_rebuild = !index_exists || !index_is_valid || has_deletions;

    // ── Step 2: Load E5 model ──────────────────────────────────────────────────
    println!("🧠 Loading E5 model...");
    let load_start = std::time::Instant::now();

    let mut bi_session = {
        let builder = Session::builder()
            .map_err(|e| CliError::from(format!("Session error: {}", e)))?;
        
        // Try to add CUDA provider - it returns the provider directly, not a Result
        let cuda_provider = CUDAExecutionProvider::default().build();
        let builder_with_cuda = builder.with_execution_providers([cuda_provider]);
        
        let builder = match builder_with_cuda {
            Ok(b) => {
                println!("  ✓ Using GPU acceleration");
                b
            }
            Err(e) => {
                println!("  ⚠ GPU not available ({}), using CPU", e);
                // Recreate builder without CUDA
                Session::builder()
                    .map_err(|e| CliError::from(format!("Session error: {}", e)))?
            }
        };
        
        builder
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| CliError::from(format!("Opt error: {}", e)))?
            .with_intra_threads(4)
            .map_err(|e| CliError::from(format!("Thread error: {}", e)))?
            .commit_from_file(bi_model_path)
            .map_err(|e| CliError::from(format!("Model load error: {}", e)))?
    };

    let bi_tokenizer = Tokenizer::from_file(bi_tokenizer_path)
        .map_err(|e| CliError::from(format!("Tokenizer error: {}", e)))?;

    println!("  ✓ Model loaded in {:.2}s", load_start.elapsed().as_secs_f32());

    // Embedding function
    let embed_batch = |session: &mut Session,
                       tokenizer: &Tokenizer,
                       texts: &[String]|
     -> Result<Vec<Vec<f32>>, CliError> {
        let batch_size = texts.len();
        if batch_size == 0 {
            return Ok(vec![]);
        }

        let mut all_ids = vec![0i64; batch_size * BI_MAX_LEN];
        let mut all_mask = vec![0i64; batch_size * BI_MAX_LEN];
        let mut all_types = vec![0i64; batch_size * BI_MAX_LEN];
        let mut lengths = vec![0usize; batch_size];

        for (b, text) in texts.iter().enumerate() {
            let prefixed = format!("passage: {}", text);
            let encoding = tokenizer
                .encode(prefixed.as_str(), true)
                .map_err(|e| CliError::from(format!("Tokenization failed: {}", e)))?;

            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let types = encoding.get_type_ids();
            let len = ids.len().min(BI_MAX_LEN);
            lengths[b] = len;

            let offset = b * BI_MAX_LEN;
            for i in 0..len {
                all_ids[offset + i] = ids[i] as i64;
                all_mask[offset + i] = mask[i] as i64;
                all_types[offset + i] = types[i] as i64;
            }
        }

        let id_tensor = Value::from_array(([batch_size, BI_MAX_LEN], all_ids))
            .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?;
        let mask_tensor = Value::from_array(([batch_size, BI_MAX_LEN], all_mask))
            .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?;
        let type_tensor = Value::from_array(([batch_size, BI_MAX_LEN], all_types))
            .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?;

        let outputs = session
            .run(ort::inputs![
                "input_ids" => id_tensor,
                "attention_mask" => mask_tensor,
                "token_type_ids" => type_tensor,
            ])
            .map_err(|e| CliError::from(format!("Inference failed: {}", e)))?;

        let (_, emb) = outputs["last_hidden_state"]
            .try_extract_tensor::<f32>()
            .map_err(|e| CliError::from(format!("Extract failed: {}", e)))?;

        let mut results = Vec::with_capacity(batch_size);
        for b in 0..batch_size {
            let len = lengths[b];
            let mut pooled = vec![0.0f32; BI_HIDDEN];
            
            for i in 0..len {
                let base = b * BI_MAX_LEN * BI_HIDDEN + i * BI_HIDDEN;
                for j in 0..BI_HIDDEN {
                    let idx = base + j;
                    if idx < emb.len() {
                        pooled[j] += emb[idx];
                    }
                }
            }
            
            if len > 0 {
                for v in &mut pooled {
                    *v /= len as f32;
                }
            }
            let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut pooled {
                    *v /= norm;
                }
            }
            results.push(pooled);
        }
        Ok(results)
    };

    // ── Step 3: Build or update index ──────────────────────────────────────────
    if needs_full_rebuild || !needs_reindex.is_empty() {
        #[derive(serde::Serialize, serde::Deserialize, Clone)]
        struct ChunkRecord {
            file_path: String,
            parent_chunk: String,
            child_text: String,
        }

        let files_to_index: Vec<&(String, String)> = if needs_full_rebuild {
            println!("📚 Building search index ({} files)...", current_files.len());
            current_files.iter().collect()
        } else {
            println!("🔄 Updating index ({} changed files)...", needs_reindex.len());
            needs_reindex
        };

        const BATCH_SIZE: usize = 32;

        let mut all_chunks: Vec<ChunkRecord> = Vec::new();
        let mut pending_chunks: Vec<ChunkRecord> = Vec::new();

        // Create chunks
        println!("✂️  Creating chunks...");
        for (file_path, content) in &files_to_index {
            let parent_chunks = chunk_text(content, PARENT_CHARS, PARENT_OVERLAP);
            for parent in &parent_chunks {
                let child_chunks = chunk_text(parent, CHILD_CHARS, CHILD_OVERLAP);
                for child in child_chunks {
                    if !child.trim().is_empty() {
                        pending_chunks.push(ChunkRecord {
                            file_path: file_path.clone(),
                            parent_chunk: parent.clone(),
                            child_text: child,
                        });
                    }
                }
            }
            manifest.insert(file_path.clone(), hash_content(content));
        }

        if pending_chunks.is_empty() {
            println!("No chunks created.");
        } else {
            println!("📊 Embedding {} chunks...", pending_chunks.len());
            
            let mut all_vectors = Vec::new();
            let total_batches = (pending_chunks.len() + BATCH_SIZE - 1) / BATCH_SIZE;
            let embed_start = std::time::Instant::now();
            
            for (batch_num, batch) in pending_chunks.chunks(BATCH_SIZE).enumerate() {
                if batch_num % 10 == 0 {
                    print!("\r  Progress: {}/{} batches ({:.0}%)", 
                           batch_num + 1, total_batches,
                           (batch_num as f32 / total_batches as f32) * 100.0);
                }
                
                let texts: Vec<String> = batch.iter().map(|c| c.child_text.clone()).collect();
                match embed_batch(&mut bi_session, &bi_tokenizer, &texts) {
                    Ok(vecs) => {
                        for (rec, vec) in batch.iter().zip(vecs.into_iter()) {
                            all_vectors.extend_from_slice(&vec);
                            all_chunks.push(ChunkRecord {
                                file_path: rec.file_path.clone(),
                                parent_chunk: rec.parent_chunk.clone(),
                                child_text: rec.child_text.clone(),
                            });
                        }
                    }
                    Err(e) => eprintln!("\nWarning: batch embed failed: {:?}", e),
                }
            }
            println!("\r  ✓ Embedding completed in {:.2}s           ", embed_start.elapsed().as_secs_f32());

            // Save index
            println!("💾 Saving index...");
            std::fs::create_dir_all(&index_dir)
                .map_err(|e| CliError::from(format!("Cannot create index dir: {}", e)))?;

            let bytes: Vec<u8> = all_vectors.iter().flat_map(|f| f.to_le_bytes()).collect();
            std::fs::write(&vectors_path, &bytes)
                .map_err(|e| CliError::from(format!("Cannot write vectors: {}", e)))?;

            let chunks_json = serde_json::to_string(&all_chunks)
                .map_err(|e| CliError::from(format!("Cannot serialize chunks: {}", e)))?;
            std::fs::write(&chunks_path, chunks_json)
                .map_err(|e| CliError::from(format!("Cannot write chunks: {}", e)))?;

            manifest.retain(|p, _| current_paths.contains(p.as_str()));
            let manifest_json = serde_json::to_string(&manifest)
                .map_err(|e| CliError::from(format!("Cannot serialize manifest: {}", e)))?;
            std::fs::write(&manifest_path, manifest_json)
                .map_err(|e| CliError::from(format!("Cannot write manifest: {}", e)))?;

            println!("  ✓ Index saved ({} total chunks)", all_chunks.len());
        }
    } else {
        println!("📖 Index is up to date, loading...");
    }

    // ── Step 4: Load index from disk ───────────────────────────────────────────
    #[derive(serde::Serialize, serde::Deserialize)]
    struct ChunkRecord {
        file_path: String,
        parent_chunk: String,
        child_text: String,
    }

    let chunks_raw = std::fs::read_to_string(&chunks_path)
        .map_err(|e| CliError::from(format!("Cannot read index: {}", e)))?;
    let chunks: Vec<ChunkRecord> = serde_json::from_str(&chunks_raw)
        .map_err(|e| CliError::from(format!("Invalid index: {}", e)))?;

    let vector_bytes = std::fs::read(&vectors_path)
        .map_err(|e| CliError::from(format!("Cannot read vectors: {}", e)))?;

    if vector_bytes.len() % 4 != 0 {
        return Err(CliError::from("Corrupted index"));
    }

    let vectors: Vec<f32> = vector_bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    let num_chunks = chunks.len();
    if vectors.len() != num_chunks * BI_HIDDEN {
        return Err(CliError::from("Index corrupted"));
    }

    if num_chunks == 0 {
        println!("No documents indexed yet.");
        return Ok(());
    }

    println!("  ✓ Loaded {} chunks", num_chunks);

    // ── Step 5: Embed the query ─────────────────────────────────────────────────
    println!("🔎 Searching...");
    
    let prefixed = format!("query: {}", query);
    let encoding = bi_tokenizer
        .encode(prefixed.as_str(), true)
        .map_err(|e| CliError::from(format!("Tokenization failed: {}", e)))?;

    let ids = encoding.get_ids();
    let mask = encoding.get_attention_mask();
    let types = encoding.get_type_ids();

    let mut padded_ids = vec![0i64; BI_MAX_LEN];
    let mut padded_mask = vec![0i64; BI_MAX_LEN];
    let mut padded_types = vec![0i64; BI_MAX_LEN];

    let qlen = ids.len().min(BI_MAX_LEN);
    for i in 0..qlen {
        padded_ids[i] = ids[i] as i64;
        padded_mask[i] = mask[i] as i64;
        padded_types[i] = types[i] as i64;
    }

    let id_tensor = Value::from_array(([1usize, BI_MAX_LEN], padded_ids))
        .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?;
    let mask_tensor = Value::from_array(([1usize, BI_MAX_LEN], padded_mask))
        .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?;
    let type_tensor = Value::from_array(([1usize, BI_MAX_LEN], padded_types))
        .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?;

    let outputs = bi_session
        .run(ort::inputs![
            "input_ids" => id_tensor,
            "attention_mask" => mask_tensor,
            "token_type_ids" => type_tensor,
        ])
        .map_err(|e| CliError::from(format!("Inference failed: {}", e)))?;

    let (_, q_emb) = outputs["last_hidden_state"]
        .try_extract_tensor::<f32>()
        .map_err(|e| CliError::from(format!("Extract failed: {}", e)))?;

    let mut query_vec = vec![0.0f32; BI_HIDDEN];
    for i in 0..qlen {
        let base = i * BI_HIDDEN;
        for j in 0..BI_HIDDEN {
            let idx = base + j;
            if idx < q_emb.len() {
                query_vec[j] += q_emb[idx];
            }
        }
    }
    if qlen > 0 {
        for v in &mut query_vec {
            *v /= qlen as f32;
        }
    }
    let norm: f32 = query_vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut query_vec {
            *v /= norm;
        }
    }

    // ── Step 6: Compute similarities and get results ───────────────────────────
    let mut scores: Vec<(usize, f32)> = (0..num_chunks)
        .map(|i| {
            let base = i * BI_HIDDEN;
            let sim: f32 = (0..BI_HIDDEN).map(|j| query_vec[j] * vectors[base + j]).sum();
            (i, sim)
        })
        .collect();

    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Deduplicate by file path
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    
    for (idx, score) in scores.iter() {
        let path = chunks[*idx].file_path.clone();
        if seen.insert(path.clone()) {
            results.push((path, *score));
        }
        if results.len() >= TOP_K {
            break;
        }
    }

    // ── Step 7: Display results ─────────────────────────────────────────────────
    if results.is_empty() {
        println!("\n{}", "No results found.".yellow());
    } else {
        println!("\n{}", "Top Results:".green().bold());
        println!("{}", "============".green());
        
        for (i, (path, score)) in results.iter().enumerate() {
            let score_percent = (score * 100.0) as u8;
            let bar_len = (score_percent / 2) as usize;
            let bar = "█".repeat(bar_len);
            let empty_bar = "░".repeat(50 - bar_len);
            
            let score_colored = if *score > 0.7 {
                format!("{:.4}", score).green()
            } else if *score > 0.4 {
                format!("{:.4}", score).yellow()
            } else {
                format!("{:.4}", score).red()
            };
            
            println!("\n{}. {}", i + 1, path.cyan().bold());
            println!("   Score: {} ({:.1}%)", score_colored, score_percent);
            println!("   Relevance: [{}{}]", bar, empty_bar);
        }
    }

    println!("\n✅ Search completed in {:.2}s", start_time.elapsed().as_secs_f32());
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