use std::path::Path;
use std::collections::{HashMap, HashSet};
use cli_rs::cli_error::{CliError, CliResult};
use colored::Colorize;
use ort::execution_providers::{
    CUDAExecutionProvider,
    CoreMLExecutionProvider,
    DirectMLExecutionProvider,
    ROCmExecutionProvider,
    CPUExecutionProvider,
};
use ort::execution_providers::coreml::ComputeUnits;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Value;
use serde::{Deserialize, Serialize};
use tokenizers::Tokenizer;
use lb_rs::Uuid;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

use crate::core;
use crate::ensure_account_and_root;

// ── Model management ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ModelType {
    Embedder,
    Reranker,
    Generative,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub name: String,
    pub kind: ModelType,
    pub location: String,
    pub url: String,
}

impl ModelMetadata {
    pub fn new(name: &str, kind: ModelType, location: &str, url: &str) -> Self {
        Self {
            name: name.to_string(),
            kind,
            location: location.to_string(),
            url: url.to_string(),
        }
    }
}

fn get_models() -> Vec<ModelMetadata> {
    vec![
        ModelMetadata::new(
            "multilingual-e5-large",
            ModelType::Embedder,
            "multilingual-e5-large",
            "https://huggingface.co/intfloat/multilingual-e5-large/resolve/main/"
        ),
        ModelMetadata::new(
            "ms-marco-MiniLM-L-6-v2",
            ModelType::Reranker,
            "ms-marco-MiniLM-L-6-v2",
            "https://huggingface.co/cross-encoder/ms-marco-MiniLM-L-6-v2/resolve/main/"
        ),
    ]
}

async fn ensure_all_models_downloaded(lb: &crate::Lb) -> CliResult<()> {
    let models = get_models();
    let writeable_path = std::path::PathBuf::from(&lb.config.writeable_path);
    let cache_dir = writeable_path.join("models");
    for model in models {
        ensure_model_downloaded(&cache_dir, &model).await?;
    }
    Ok(())
}

// FIX: check file size, not just existence. A truncated/partial download
// passes an exists() check and causes the session to hang indefinitely.
fn file_is_valid(path: &std::path::Path, min_bytes: u64) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() >= min_bytes)
        .unwrap_or(false)
}

async fn ensure_model_downloaded(cache_dir: &std::path::Path, model: &ModelMetadata) -> CliResult<()> {
    let model_dir = cache_dir.join(&model.location);
    let model_path = model_dir.join("model.onnx");
    let model_data_path = model_dir.join("model.onnx_data");
    let tokenizer_path = model_dir.join("tokenizer.json");

    println!("Checking model paths: {}", model_dir.display());
    println!("  model: {}", model_path.display());
    println!("  model_data: {}", model_data_path.display());
    println!("  tokenizer: {}", tokenizer_path.display());

    let model_exists = match model.kind {
        ModelType::Reranker   => file_is_valid(&model_path,      10_000_000)   // ~90 MB expected
                              && file_is_valid(&tokenizer_path,       100_000),
        ModelType::Embedder   => file_is_valid(&model_path,     100_000_000)   // ~340 MB expected
                              && file_is_valid(&model_data_path, 400_000_000)  // ~560 MB expected
                              && file_is_valid(&tokenizer_path,       100_000),
        ModelType::Generative => file_is_valid(&model_path,      10_000_000),
    };
    if model_exists { return Ok(()); }

    println!("📥 Downloading {} from Hugging Face...", model.name);
    if matches!(model.kind, ModelType::Embedder) {
        println!("   This is a large model — only downloaded once.");
    }

    std::fs::create_dir_all(&model_dir)
        .map_err(|e| CliError::from(format!("Cannot create model dir: {}", e)))?;

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| CliError::from(format!("HTTP client error: {}", e)))?;

    let possible_urls = match model.name.as_str() {
        "multilingual-e5-large" => vec![
            format!("{}onnx/model.onnx", model.url),
            "https://huggingface.co/Xenova/multilingual-e5-large/resolve/main/onnx/model.onnx".to_string(),
        ],
        "ms-marco-MiniLM-L-6-v2" => vec![
            format!("{}onnx/model.onnx", model.url),
            "https://huggingface.co/Xenova/ms-marco-MiniLM-L-6-v2/resolve/main/onnx/model.onnx".to_string(),
        ],
        _ => vec![format!("{}model.onnx", model.url)],
    };

    if !model_path.exists() {
        let mut downloaded = false;
        for url in &possible_urls {
            if download_file(&client, url, &model_path, &format!("{} model", model.name)).await.is_ok() {
                downloaded = true;
                break;
            }
        }
        if !downloaded {
            return Err(CliError::from(format!("Failed to download {} model from any source", model.name)));
        }
    }

    if matches!(model.kind, ModelType::Embedder) && !model_data_path.exists() {
        let possible_data_urls = vec![
            format!("{}onnx/model.onnx_data", model.url),
            "https://huggingface.co/Xenova/multilingual-e5-large/resolve/main/onnx/model.onnx_data".to_string(),
        ];
        let mut downloaded = false;
        for url in &possible_data_urls {
            if download_file(&client, url, &model_data_path, &format!("{} model data", model.name)).await.is_ok() {
                downloaded = true;
                break;
            }
        }
        if !downloaded {
            return Err(CliError::from(format!("Failed to download {} model data from any source", model.name)));
        }
    }

    if matches!(model.kind, ModelType::Embedder | ModelType::Reranker) && !tokenizer_path.exists() {
        let possible_tokenizer_urls = match model.name.as_str() {
            "multilingual-e5-large" => vec![
                format!("{}tokenizer.json", model.url),
                "https://huggingface.co/Xenova/multilingual-e5-large/resolve/main/tokenizer.json".to_string(),
            ],
            "ms-marco-MiniLM-L-6-v2" => vec![
                format!("{}tokenizer.json", model.url),
                "https://huggingface.co/Xenova/ms-marco-MiniLM-L-6-v2/resolve/main/tokenizer.json".to_string(),
            ],
            _ => vec![format!("{}tokenizer.json", model.url)],
        };
        let mut downloaded = false;
        for url in &possible_tokenizer_urls {
            if download_file(&client, url, &tokenizer_path, &format!("{} tokenizer", model.name)).await.is_ok() {
                downloaded = true;
                break;
            }
        }
        if !downloaded {
            return Err(CliError::from(format!("Failed to download {} tokenizer from any source", model.name)));
        }
    }

    println!("  ✓ {} ready", model.name);
    Ok(())
}

async fn download_file(client: &reqwest::Client, url: &str, dest: &std::path::Path, label: &str) -> CliResult<()> {
    println!("   Downloading {}...", label);
    let resp = client.get(url).send().await
        .map_err(|e| CliError::from(format!("Request failed for {}: {}", label, e)))?;
    if !resp.status().is_success() {
        return Err(CliError::from(format!("HTTP {} downloading {}", resp.status().as_u16(), label)));
    }
    let bytes = resp.bytes().await
        .map_err(|e| CliError::from(format!("Read failed for {}: {}", label, e)))?;
    println!("   Writing {} to {}", label, dest.display());
    std::fs::write(dest, &bytes)
        .map_err(|e| CliError::from(format!("Write failed for {} -> {}: {}", label, dest.display(), e)))?;
    println!("  ✓ {} saved ({:.1} MB)", label, bytes.len() as f64 / 1_048_576.0);
    Ok(())
}

// ── Session builder ───────────────────────────────────────────────────────────

fn build_session(model_path: &std::path::Path) -> CliResult<Session> {
    // Normalize path separators for Windows - mixed separators cause ONNX Runtime
    // to silently fail to find model.onnx_data, hanging indefinitely on load.
    let model_path_buf = model_path.canonicalize()
        .map_err(|e| CliError::from(format!("Cannot resolve model path {}: {}", model_path.display(), e)))?;
    let model_path = model_path_buf.as_path();

    let cpu_threads = {
        let n = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);
        n.min(8)
    };
    type BuildFn<'a> = Box<dyn Fn() -> CliResult<Session> + 'a>;
    let candidates: Vec<(&str, BuildFn<'_>)> = vec![
        ("CUDA", Box::new(|| {
            let mut builder = Session::builder()
                .map_err(|e| CliError::from(format!("Builder error: {}", e)))?;
            builder = builder
                .with_execution_providers([CUDAExecutionProvider::default().build()])
                .map_err(|e| CliError::from(format!("CUDA provider error: {}", e)))?;
            builder = builder
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| CliError::from(format!("Opt level error: {}", e)))?;
            builder = builder
                .with_intra_threads(4)
                .map_err(|e| CliError::from(format!("Thread error: {}", e)))?;
            println!("Trying CUDA commit_from_file: {}", model_path.display());
            builder.commit_from_file(model_path)
                .map_err(|e| CliError::from(format!("Model load error: {}", e)))
        })),
        ("ROCm", Box::new(|| {
            let mut builder = Session::builder()
                .map_err(|e| CliError::from(format!("Builder error: {}", e)))?;
            builder = builder
                .with_execution_providers([ROCmExecutionProvider::default().build()])
                .map_err(|e| CliError::from(format!("ROCm provider error: {}", e)))?;
            builder = builder
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| CliError::from(format!("Opt level error: {}", e)))?;
            builder = builder
                .with_intra_threads(4)
                .map_err(|e| CliError::from(format!("Thread error: {}", e)))?;
            println!("Trying ROCm commit_from_file: {}", model_path.display());
            builder.commit_from_file(model_path)
                .map_err(|e| CliError::from(format!("Model load error: {}", e)))
        })),
        ("CoreML", Box::new(|| {
            let mut builder = Session::builder()
                .map_err(|e| CliError::from(format!("Builder error: {}", e)))?;
            builder = builder
                .with_execution_providers([
                    CoreMLExecutionProvider::default()
                        .with_compute_units(ComputeUnits::CPUAndNeuralEngine)
                        .build()
                ])
                .map_err(|e| CliError::from(format!("CoreML provider error: {}", e)))?;
            builder = builder
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| CliError::from(format!("Opt level error: {}", e)))?;
            builder = builder
                .with_intra_threads(4)
                .map_err(|e| CliError::from(format!("Thread error: {}", e)))?;
            println!("Trying CoreML commit_from_file: {}", model_path.display());
            builder.commit_from_file(model_path)
                .map_err(|e| CliError::from(format!("Model load error: {}", e)))
        })),
        ("DirectML", Box::new(|| {
            let mut builder = Session::builder()
                .map_err(|e| CliError::from(format!("Builder error: {}", e)))?;
            builder = builder
                .with_execution_providers([DirectMLExecutionProvider::default().build()])
                .map_err(|e| CliError::from(format!("DirectML provider error: {}", e)))?;
            builder = builder
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| CliError::from(format!("Opt level error: {}", e)))?;
            builder = builder
                .with_intra_threads(4)
                .map_err(|e| CliError::from(format!("Thread error: {}", e)))?;
            println!("Trying DirectML commit_from_file: {}", model_path.display());
            builder.commit_from_file(model_path)
                .map_err(|e| CliError::from(format!("Model load error: {}", e)))
        })),
        ("CPU", Box::new(move || {
            let mut builder = Session::builder()
                .map_err(|e| CliError::from(format!("Builder error: {}", e)))?;
            builder = builder
                .with_execution_providers([CPUExecutionProvider::default().build()])
                .map_err(|e| CliError::from(format!("CPU provider error: {}", e)))?;
            builder = builder
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| CliError::from(format!("Opt level error: {}", e)))?;
            builder = builder
                .with_intra_threads(cpu_threads)
                .map_err(|e| CliError::from(format!("Thread error: {}", e)))?;
            println!("Trying CPU commit_from_file: {}", model_path.display());
            builder.commit_from_file(model_path)
                .map_err(|e| CliError::from(format!("Model load error: {}", e)))
        })),
    ];

    for (name, build) in &candidates {
        match build() {
            Ok(session) => {
                println!("  ✓ Using {} execution provider", name);
                return Ok(session);
            }
            Err(_) => continue,
        }
    }

    Err(CliError::from(
        "No execution provider could be initialized. This is likely a build configuration issue."
    ))
}

// ── Chunk record ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkRecord {
    pub file_id:      String,
    pub file_path:    String,
    pub file_name:    String,
    pub parent_chunk: String,
    pub child_text:   String,
}

// ── Index structure ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileIndex {
    pub hmac:    Option<Vec<u8>>,
    pub chunks:  Vec<ChunkRecord>,
    pub vectors: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchIndex {
    pub files:   HashMap<String, FileIndex>,
    pub version: u32,
}

impl SearchIndex {
    fn new() -> Self {
        Self { files: HashMap::new(), version: 1 }
    }
}

// ── usearch index builder ─────────────────────────────────────────────────────

fn build_hnsw(
    all_chunks:  &[ChunkRecord],
    all_vectors: &[f32],
    hidden:      usize,
) -> CliResult<Index> {
    // FIX 3: Guard against a corrupted index where vector count doesn't match
    // chunk count. Without this check, the slice below panics at runtime with
    // an unhelpful "index out of bounds" message.
    let expected_len = all_chunks.len() * hidden;
    if all_vectors.len() != expected_len {
        return Err(CliError::from(format!(
            "Vector/chunk mismatch: expected {} floats ({} chunks × {} dims), got {}. \
             The index may be corrupted — delete it and re-run to rebuild.",
            expected_len, all_chunks.len(), hidden, all_vectors.len()
        )));
    }

    let options = IndexOptions {
        dimensions: hidden,
        metric:     MetricKind::Cos,
        quantization: ScalarKind::F32,
        ..Default::default()
    };

    let index = Index::new(&options)
        .map_err(|e| CliError::from(format!("usearch index create failed: {}", e)))?;

    index.reserve(all_chunks.len())
        .map_err(|e| CliError::from(format!("usearch reserve failed: {}", e)))?;

    for (i, _) in all_chunks.iter().enumerate() {
        let base = i * hidden;
        let vec  = &all_vectors[base..base + hidden];
        index.add(i as u64, vec)
            .map_err(|e| CliError::from(format!("usearch add failed at chunk {}: {}", i, e)))?;
    }

    Ok(index)
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub async fn search_semantic(query: &str) -> CliResult<()> {
    let start_time = std::time::Instant::now();
    println!("{}", "🔍 Semantic Search".cyan().bold());
    println!("Query: {}\n", query);

    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    const BI_HIDDEN:           usize = 1024;
    const BI_MAX_LEN:          usize = 128;
    const CHILD_CHARS:         usize = 512;
    const PARENT_CHARS:        usize = 2048;
    const CHILD_OVERLAP:       usize = 51;
    const PARENT_OVERLAP:      usize = 205;
    const TOP_K:               usize = 5;
    const RERANK_K:            usize = 100;
    const MAX_CHUNKS_PER_FILE: usize = 50_000;
    const BATCH_SIZE:          usize = 32;
    const RERANKER_MAX_LEN:    usize = 512;

    let writeable_path = std::path::PathBuf::from(&lb.config.writeable_path);
    let cache_dir      = writeable_path.join("models");
    let bi_dir         = cache_dir.join("multilingual-e5-large");
    let model_path     = bi_dir.join("model.onnx");
    let tokenizer_path = bi_dir.join("tokenizer.json");

    let reranker_dir            = cache_dir.join("ms-marco-MiniLM-L-6-v2");
    let reranker_model_path     = reranker_dir.join("model.onnx");
    let reranker_tokenizer_path = reranker_dir.join("tokenizer.json");

    let index_dir  = writeable_path.join("search_index");
    let index_path = index_dir.join("index.json");

    ensure_all_models_downloaded(lb).await?;

    println!("🧠 Loading multilingual-e5-large...");
    let load_start = std::time::Instant::now();
    println!("  model path: {}", model_path.display());
    match std::fs::metadata(&model_path) {
        Ok(m) => println!("  model size: {} bytes", m.len()),
        Err(e) => println!("  model metadata error: {}", e),
    }
    let mut bi_session = build_session(&model_path)?;
    let bi_tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| CliError::from(format!("Tokenizer error: {}", e)))?;
    println!("  ✓ Embedder loaded in {:.2}s", load_start.elapsed().as_secs_f32());

    println!("🧠 Loading reranker...");
    let reranker_load_start = std::time::Instant::now();
    println!("  reranker model path: {}", reranker_model_path.display());
    match std::fs::metadata(&reranker_model_path) {
        Ok(m) => println!("  reranker model size: {} bytes", m.len()),
        Err(e) => println!("  reranker model metadata error: {}", e),
    }
    let mut reranker_session = build_session(&reranker_model_path)?;
    let reranker_tokenizer = Tokenizer::from_file(&reranker_tokenizer_path)
        .map_err(|e| CliError::from(format!("Reranker tokenizer error: {}", e)))?;
    println!("  ✓ Reranker loaded in {:.2}s", reranker_load_start.elapsed().as_secs_f32());

    println!("📁 Scanning files...");
    let files = lb.get_and_get_children_recursively(&lb.root().await?.id).await?;

    let mut search_index: SearchIndex = if index_path.exists() {
        let data = std::fs::read_to_string(&index_path)
            .map_err(|e| CliError::from(format!("Cannot read index: {}", e)))?;
        serde_json::from_str(&data).unwrap_or_else(|_| SearchIndex::new())
    } else {
        SearchIndex::new()
    };

    let mut processed_file_ids = HashSet::new();
    let mut files_to_embed: Vec<(String, String, String)> = Vec::new();

    println!("🔍 Checking for changes...");
    for file in &files {
        if file.is_folder() { continue; }
        let name = file.name.to_lowercase();
        if !name.ends_with(".txt") && !name.ends_with(".md") { continue; }

        let (hmac_opt, content_bytes) = match lb.read_document_with_hmac(file.id, false).await {
            Ok((h, c)) => (h, c),
            Err(_) => continue,
        };

        let content = String::from_utf8_lossy(&content_bytes).to_string();
        if content.trim().len() < 20 { continue; }

        let full_path = lb.get_path_by_id(file.id).await.unwrap_or_else(|_| file.name.clone());
        let file_name = Path::new(&file.name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&file.name)
            .replace(['-', '_'], " ");

        let file_id_str = file.id.to_string();
        processed_file_ids.insert(file_id_str.clone());

        let hmac_vec = hmac_opt.map(|h| h.to_vec());

        match search_index.files.get(&file_id_str) {
            Some(existing) if existing.hmac == hmac_vec => {
                println!("  ✓ Unchanged: {}", file_name);
            }
            _ => {
                println!("  📝 New/Modified: {}", file_name);
                files_to_embed.push((file_id_str, full_path, file_name));
            }
        }
    }

    let deleted_files: Vec<String> = search_index.files.keys()
        .filter(|id| !processed_file_ids.contains(*id))
        .cloned()
        .collect();

    if !deleted_files.is_empty() {
        println!("🗑️  Removing {} deleted files from index", deleted_files.len());
        for file_id in deleted_files {
            search_index.files.remove(&file_id);
        }
    }

    if !files_to_embed.is_empty() {
        println!("📊 Embedding {} new/modified files...", files_to_embed.len());

        for (file_idx, (file_id_str, full_path, file_name)) in files_to_embed.iter().enumerate() {
            let file_id = Uuid::parse_str(file_id_str)
                .map_err(|e| CliError::from(format!("Invalid UUID: {}", e)))?;

            let (hmac_opt, content_bytes) = match lb.read_document_with_hmac(file_id, false).await {
                Ok((h, c)) => (h, c),
                Err(_) => continue,
            };

            let content = String::from_utf8_lossy(&content_bytes).to_string();

            {
                use std::io::Write;
                print!("\r\x1B[K  Processing: {}/{} - {}", file_idx + 1, files_to_embed.len(), file_name);
                let _ = std::io::stdout().flush();
            }

            // Skip files that are too large — they generate thousands of chunks
            // and can stall the embedding loop for 30+ minutes.
            // 200 KB (~40 pages) is more than enough for useful search results.
            const MAX_FILE_CHARS: usize = 200_000;
            if content.len() > MAX_FILE_CHARS {
                eprintln!("\n  ⚠ Skipping {} ({} chars, limit {})", file_name, content.len(), MAX_FILE_CHARS);
                continue;
            }

            let mut chunks: Vec<ChunkRecord> = Vec::new();
            let sections = split_markdown_sections(&content);

            for section in &sections {
                let parent_chunks = chunk_text(section, PARENT_CHARS, PARENT_OVERLAP, MAX_CHUNKS_PER_FILE);
                for parent in &parent_chunks {
                    let child_chunks = chunk_text(parent, CHILD_CHARS, CHILD_OVERLAP, MAX_CHUNKS_PER_FILE);
                    for child in child_chunks {
                        if child.trim().is_empty() { continue; }
                        chunks.push(ChunkRecord {
                            file_id:      file_id_str.clone(),
                            file_path:    full_path.clone(),
                            file_name:    file_name.clone(),
                            parent_chunk: parent.clone(),
                            child_text:   child,
                        });
                    }
                }
            }

            let file_intro: String = content.chars().take(200).collect();
            chunks.push(ChunkRecord {
                file_id:      file_id_str.clone(),
                file_path:    full_path.clone(),
                file_name:    file_name.clone(),
                parent_chunk: format!("{}\n{}", file_name, file_intro),
                child_text:   file_name.clone(),
            });

            let mut all_vectors: Vec<f32> = Vec::new();
            let mut failed = false;

            for batch in chunks.chunks(BATCH_SIZE) {
                let texts: Vec<String> = batch.iter().map(|c| c.child_text.clone()).collect();
                match embed_batch(&mut bi_session, &bi_tokenizer, &texts, BI_HIDDEN, BI_MAX_LEN) {
                    Ok(vecs) => {
                        for vec in vecs { all_vectors.extend_from_slice(&vec); }
                    }
                    Err(e) => {
                        eprintln!("\nError: batch embed failed for file {}: {:?}", file_name, e);
                        failed = true;
                        break;
                    }
                }
            }

            if !failed {
                let hmac_vec = hmac_opt.map(|h| h.to_vec());
                search_index.files.insert(file_id_str.clone(), FileIndex {
                    hmac: hmac_vec,
                    chunks,
                    vectors: all_vectors,
                });
            } else {
                eprintln!("  Skipping file {} due to embedding error", file_name);
            }
        }
        println!();

        println!("💾 Saving index...");
        std::fs::create_dir_all(&index_dir)
            .map_err(|e| CliError::from(format!("Cannot create index dir: {}", e)))?;
        let index_json = serde_json::to_string(&search_index)
            .map_err(|e| CliError::from(format!("Cannot serialize index: {}", e)))?;
        std::fs::write(&index_path, index_json)
            .map_err(|e| CliError::from(format!("Cannot write index: {}", e)))?;
        println!("  ✓ Index saved ({} files indexed)", search_index.files.len());
    } else {
        println!("📖 Index is up to date ({} files)", search_index.files.len());
    }

    if search_index.files.is_empty() {
        println!("No documents indexed yet.");
        return Ok(());
    }

    // ── Collect all chunks + vectors ──────────────────────────────────────────
    let mut all_chunks: Vec<ChunkRecord> = Vec::new();
    let mut all_vectors: Vec<f32>        = Vec::new();

    for file_index in search_index.files.values() {
        for chunk in &file_index.chunks { all_chunks.push(chunk.clone()); }
        all_vectors.extend_from_slice(&file_index.vectors);
    }

    if all_chunks.is_empty() {
        println!("No chunks indexed yet.");
        return Ok(());
    }

    // ── Build HNSW index ──────────────────────────────────────────────────────
    println!("🔎 Building search index ({} chunks)...", all_chunks.len());
    let hnsw_start = std::time::Instant::now();
    let hnsw = build_hnsw(&all_chunks, &all_vectors, BI_HIDDEN)?;
    println!("  ✓ HNSW index built in {:.2}s", hnsw_start.elapsed().as_secs_f32());

    // ── Embed the query ───────────────────────────────────────────────────────
    println!("🔎 Searching...");
    let query_vec = embed_single(
        &mut bi_session, &bi_tokenizer, query, "query", BI_HIDDEN, BI_MAX_LEN,
    )?;

    let results_raw = hnsw.search(&query_vec, RERANK_K)
        .map_err(|e| CliError::from(format!("usearch search failed: {}", e)))?;

    let candidates: Vec<usize> = results_raw.keys.iter()
        .map(|&k| k as usize)
        .collect();

    // ── Rerank ────────────────────────────────────────────────────────────────
    println!("  ✓ Reranking {} candidates...", candidates.len());
    let rerank_start = std::time::Instant::now();
    let mut reranked: Vec<(String, String, f32)> = Vec::new();

    for idx in &candidates {
        let chunk = &all_chunks[*idx];
        let score = rerank_pair(
            &mut reranker_session,
            &reranker_tokenizer,
            query,
            &chunk.parent_chunk,
            RERANKER_MAX_LEN,
        )?;
        reranked.push((chunk.file_path.clone(), chunk.child_text.clone(), score));
    }

    reranked.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    println!("  ✓ Reranking done in {:.2}s", rerank_start.elapsed().as_secs_f32());

    let mut seen:    HashSet<String>            = HashSet::new();
    let mut results: Vec<(String, String, f32)> = Vec::new();
    for (path, snippet, score) in &reranked {
        if seen.insert(path.clone()) {
            results.push((path.clone(), snippet.clone(), *score));
        }
        if results.len() >= TOP_K { break; }
    }

    if results.is_empty() {
        println!("\n{}", "No results found.".yellow());
    } else {
        println!("\n{}", "Top Results:".green().bold());
        println!("{}", "============".green());
        for (i, (path, snippet, score)) in results.iter().enumerate() {
            let sig           = 1.0f32 / (1.0 + (-score).exp());
            let score_percent = (sig * 100.0).clamp(0.0, 100.0) as usize;
            let bar_len       = score_percent / 2;
            let bar           = "█".repeat(bar_len);
            let empty         = "░".repeat(50usize.saturating_sub(bar_len));
            let score_colored = if sig > 0.7      { format!("{:.4}", sig).green() }
                                else if sig > 0.4 { format!("{:.4}", sig).yellow() }
                                else              { format!("{:.4}", sig).red() };
            let preview: String = snippet.chars().take(150).collect();
            let preview = preview.trim().replace('\n', " ");
            println!("\n{}. {}", i + 1, path.cyan().bold());
            println!("   Score: {} ({:.1}%)", score_colored, score_percent);
            println!("   Relevance: [{}{}]", bar, empty);
            println!("   ↳ \"{}...\"", preview.dimmed());
        }
    }

    println!("\n✅ Search completed in {:.2}s", start_time.elapsed().as_secs_f32());
    Ok(())
}

// ── Reranker ──────────────────────────────────────────────────────────────────

fn rerank_pair(
    session:   &mut Session,
    tokenizer: &Tokenizer,
    query:     &str,
    passage:   &str,
    max_len:   usize,
) -> CliResult<f32> {
    let enc = tokenizer.encode((query, passage), true)
        .map_err(|e| CliError::from(format!("Reranker tokenization failed: {}", e)))?;
    let ids   = enc.get_ids();
    let mask  = enc.get_attention_mask();
    let types = enc.get_type_ids();
    let len   = ids.len().min(max_len);
    let mut padded_ids   = vec![0i64; max_len];
    let mut padded_mask  = vec![0i64; max_len];
    let mut padded_types = vec![0i64; max_len];
    for i in 0..len {
        padded_ids[i]   = ids[i]   as i64;
        padded_mask[i]  = mask[i]  as i64;
        padded_types[i] = types[i] as i64;
    }
    let outputs = session.run(ort::inputs![
        "input_ids"      => Value::from_array(([1usize, max_len], padded_ids))
            .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?,
        "attention_mask" => Value::from_array(([1usize, max_len], padded_mask))
            .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?,
        "token_type_ids" => Value::from_array(([1usize, max_len], padded_types))
            .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?,
    ]).map_err(|e| CliError::from(format!("Reranker inference failed: {}", e)))?;

    // FIX 2: The Xenova ONNX export of ms-marco-MiniLM-L-6-v2 names its output
    // "logits" in most builds, but some quantized or older exports use "output_0".
    // Using index-based access ["logits"] panics at runtime if the key is wrong.
    // We now try both names and surface a clear error if neither is found.
    let raw_output = outputs.get("logits")
        .or_else(|| outputs.get("output_0"))
        .ok_or_else(|| CliError::from(
            "Reranker output key not found. Expected 'logits' or 'output_0'. \
             Inspect the model with Netron to find the correct output name."
        ))?;

    let (_, logits) = raw_output.try_extract_tensor::<f32>()
        .map_err(|e| CliError::from(format!("Reranker extract failed: {}", e)))?;
    Ok(logits[0])
}

// ── Embedding ─────────────────────────────────────────────────────────────────

fn embed_batch(
    session:   &mut Session,
    tokenizer: &Tokenizer,
    texts:     &[String],
    hidden:    usize,
    max_len:   usize,
) -> CliResult<Vec<Vec<f32>>> {
    let batch_size = texts.len();
    if batch_size == 0 { return Ok(vec![]); }

    let mut all_ids   = vec![0i64; batch_size * max_len];
    let mut all_mask  = vec![0i64; batch_size * max_len];
    let mut lengths   = vec![0usize; batch_size];

    for (b, text) in texts.iter().enumerate() {
        let prefixed = format!("passage: {}", text);
        let enc = tokenizer.encode(prefixed.as_str(), true)
            .map_err(|e| CliError::from(format!("Tokenization failed: {}", e)))?;
        let ids  = enc.get_ids();
        let mask = enc.get_attention_mask();
        let len  = ids.len().min(max_len);
        lengths[b] = len;
        let offset = b * max_len;
        for i in 0..len {
            all_ids[offset + i]  = ids[i]  as i64;
            all_mask[offset + i] = mask[i] as i64;
        }
    }

    // NOTE: Xenova/multilingual-e5-large ONNX only accepts input_ids and
    // attention_mask — it does NOT have a token_type_ids input node.
    let outputs = session.run(ort::inputs![
        "input_ids"      => Value::from_array(([batch_size, max_len], all_ids))
            .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?,
        "attention_mask" => Value::from_array(([batch_size, max_len], all_mask))
            .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?,
    ]).map_err(|e| CliError::from(format!("Inference failed: {}", e)))?;

    let (_, emb) = outputs["last_hidden_state"].try_extract_tensor::<f32>()
        .map_err(|e| CliError::from(format!("Extract failed: {}", e)))?;

    let mut results = Vec::with_capacity(batch_size);
    for b in 0..batch_size {
        let len = lengths[b];
        let mut pooled = vec![0.0f32; hidden];
        for i in 0..len {
            let base = b * max_len * hidden + i * hidden;
            for j in 0..hidden { pooled[j] += emb[base + j]; }
        }
        if len > 0 { for v in &mut pooled { *v /= len as f32; } }
        let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-9 { for v in &mut pooled { *v /= norm; } }
        results.push(pooled);
    }
    Ok(results)
}

fn embed_single(
    session:   &mut Session,
    tokenizer: &Tokenizer,
    text:      &str,
    prefix:    &str,
    hidden:    usize,
    max_len:   usize,
) -> CliResult<Vec<f32>> {
    let prefixed = format!("{}: {}", prefix, text);
    let enc = tokenizer.encode(prefixed.as_str(), true)
        .map_err(|e| CliError::from(format!("Tokenization failed: {}", e)))?;
    let ids  = enc.get_ids();
    let mask = enc.get_attention_mask();
    let qlen = ids.len().min(max_len);
    let mut padded_ids  = vec![0i64; max_len];
    let mut padded_mask = vec![0i64; max_len];
    for i in 0..qlen {
        padded_ids[i]  = ids[i]  as i64;
        padded_mask[i] = mask[i] as i64;
    }
    // NOTE: Xenova/multilingual-e5-large ONNX only accepts input_ids and
    // attention_mask — no token_type_ids input node exists in this export.
    let outputs = session.run(ort::inputs![
        "input_ids"      => Value::from_array(([1usize, max_len], padded_ids))
            .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?,
        "attention_mask" => Value::from_array(([1usize, max_len], padded_mask))
            .map_err(|e| CliError::from(format!("Tensor error: {}", e)))?,
    ]).map_err(|e| CliError::from(format!("Inference failed: {}", e)))?;
    let (_, q_emb) = outputs["last_hidden_state"].try_extract_tensor::<f32>()
        .map_err(|e| CliError::from(format!("Extract failed: {}", e)))?;
    let mut vec = vec![0.0f32; hidden];
    for i in 0..qlen {
        let base = i * hidden;
        for j in 0..hidden { vec[j] += q_emb[base + j]; }
    }
    if qlen > 0 { for v in &mut vec { *v /= qlen as f32; } }
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-9 { for v in &mut vec { *v /= norm; } }
    Ok(vec)
}

// ── Text utilities ────────────────────────────────────────────────────────────

fn split_markdown_sections(text: &str) -> Vec<String> {
    let mut sections: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.starts_with('#') && !current.trim().is_empty() {
            sections.push(current.trim().to_string());
            current = String::new();
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.trim().is_empty() { sections.push(current.trim().to_string()); }
    if sections.is_empty() { sections.push(text.trim().to_string()); }
    sections
}

fn chunk_text(text: &str, size: usize, overlap: usize, max_chunks: usize) -> Vec<String> {
    let chars: Vec<char> = text.trim().chars().collect();
    let total = chars.len();
    if total == 0 { return vec![]; }
    if total <= size { return vec![chars.iter().collect()]; }
    let mut chunks = Vec::new();
    let mut start  = 0usize;
    let step       = size.saturating_sub(overlap).max(1);
    while start < total && chunks.len() < max_chunks {
        let end   = (start + size).min(total);
        let chunk: String = chars[start..end].iter().collect();
        let chunk = chunk.trim().to_string();
        if !chunk.is_empty() { chunks.push(chunk); }
        if end >= total { break; }
        start += step;
    }
    chunks
}