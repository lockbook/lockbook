use std::path::Path;
use std::collections::{HashMap, HashSet};
use cli_rs::cli_error::{CliError, CliResult};
use colored::Colorize;
use ort::execution_providers::CUDAExecutionProvider;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Value;
use serde::{Deserialize, Serialize};
use tokenizers::Tokenizer;
use lb_rs::Uuid;

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

async fn ensure_model_downloaded(cache_dir: &std::path::Path, model: &ModelMetadata) -> CliResult<()> {
    let model_dir = cache_dir.join(&model.location);
    let model_path = model_dir.join("model.onnx");
    let model_data_path = model_dir.join("model.onnx_data");
    let tokenizer_path = model_dir.join("tokenizer.json");
    
    // Check if model already exists
    let model_exists = match model.kind {
        ModelType::Reranker => model_path.exists() && tokenizer_path.exists(),
        ModelType::Embedder => model_path.exists() && model_data_path.exists() && tokenizer_path.exists(),
        ModelType::Generative => model_path.exists(),
    };
    
    if model_exists {
        return Ok(());
    }
    
    println!("📥 Downloading {} from Hugging Face...", model.name);
    if matches!(model.kind, ModelType::Embedder) {
        println!("   This is a large model — only downloaded once.");
    }
    
    std::fs::create_dir_all(&model_dir)
        .map_err(|e| CliError::from(format!("Cannot create model dir: {}", e)))?;
    
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| CliError::from(format!("HTTP client error: {}", e)))?;
    
    // Try different possible URLs for the model
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
    
    // Download model.onnx
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
    
    // Download model.onnx_data for embedder models
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
    
    // Download tokenizer.json for embedder and reranker
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
    std::fs::write(dest, &bytes)
        .map_err(|e| CliError::from(format!("Write failed for {}: {}", label, e)))?;
    println!("  ✓ {} saved ({:.1} MB)", label, bytes.len() as f64 / 1_048_576.0);
    Ok(())
}

// ── Session builder ───────────────────────────────────────────────────────────

fn build_session(model_path: &std::path::Path) -> CliResult<Session> {
    let builder = Session::builder()
        .map_err(|e| CliError::from(format!("Session builder error: {}", e)))?;
    let cuda = CUDAExecutionProvider::default().build();
    let builder = match builder.with_execution_providers([cuda]) {
        Ok(b) => {
            println!("  ✓ GPU acceleration available");
            b
        }
        Err(_) => {
            println!("  ⚠ No GPU, using CPU");
            Session::builder()
                .map_err(|e| CliError::from(format!("Session builder error: {}", e)))?
        }
    };
    builder
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| CliError::from(format!("Opt level error: {}", e)))?
        .with_intra_threads(4)
        .map_err(|e| CliError::from(format!("Thread error: {}", e)))?
        .commit_from_file(model_path)
        .map_err(|e| CliError::from(format!("Model load error: {}", e)))
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
    pub hmac: Option<Vec<u8>>,  // Store as Vec<u8> for serialization
    pub chunks: Vec<ChunkRecord>,
    pub vectors: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchIndex {
    pub files: HashMap<String, FileIndex>,
    pub version: u32,
}

impl SearchIndex {
    fn new() -> Self {
        Self {
            files: HashMap::new(),
            version: 1,
        }
    }
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
    let cache_dir = writeable_path.join("models");
    let bi_dir = cache_dir.join("multilingual-e5-large");
    let model_path = bi_dir.join("model.onnx");
    let tokenizer_path = bi_dir.join("tokenizer.json");

    let reranker_dir = cache_dir.join("ms-marco-MiniLM-L-6-v2");
    let reranker_model_path = reranker_dir.join("model.onnx");
    let reranker_tokenizer_path = reranker_dir.join("tokenizer.json");

    let index_dir = writeable_path.join("search_index");
    let index_path = index_dir.join("index.json");

    // Download all models
    ensure_all_models_downloaded(lb).await?;

    // Load models once and reuse them
    println!("🧠 Loading multilingual-e5-large...");
    let load_start = std::time::Instant::now();
    let mut bi_session = build_session(&model_path)?;
    let bi_tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| CliError::from(format!("Tokenizer error: {}", e)))?;
    println!("  ✓ Embedder loaded in {:.2}s", load_start.elapsed().as_secs_f32());

    println!("🧠 Loading reranker...");
    let reranker_load_start = std::time::Instant::now();
    let mut reranker_session = build_session(&reranker_model_path)?;
    let reranker_tokenizer = Tokenizer::from_file(&reranker_tokenizer_path)
        .map_err(|e| CliError::from(format!("Reranker tokenizer error: {}", e)))?;
    println!("  ✓ Reranker loaded in {:.2}s", reranker_load_start.elapsed().as_secs_f32());

    println!("📁 Scanning files...");
    let files = lb.get_and_get_children_recursively(&lb.root().await?.id).await?;

    // Load existing index or create new one
    let mut search_index = if index_path.exists() {
        let index_data = std::fs::read_to_string(&index_path)
            .map_err(|e| CliError::from(format!("Cannot read index: {}", e)))?;
        serde_json::from_str(&index_data).unwrap_or_else(|_| SearchIndex::new())
    } else {
        SearchIndex::new()
    };

    // Track which files we've processed
    let mut processed_file_ids = HashSet::new();
    // Store only what we need: (file_id, file_path, file_name)
    let mut files_to_embed: Vec<(String, String, String)> = Vec::new();

    println!("🔍 Checking for changes...");
    for file in &files {
        if file.is_folder() { continue; }
        let name = file.name.to_lowercase();
        if !name.ends_with(".txt") && !name.ends_with(".md") { continue; }

        // Fix: HMAC comes first, content second
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

        // Convert HMAC to Vec<u8> for comparison
        let hmac_vec = hmac_opt.map(|h| h.to_vec());
        
        // Check if file exists in index and if HMAC matches
        match search_index.files.get(&file_id_str) {
            Some(existing) if existing.hmac == hmac_vec => {
                // File unchanged - keep existing embeddings
                println!("  ✓ Unchanged: {}", file_name);
            }
            _ => {
                // New or modified file - needs embedding
                println!("  📝 New/Modified: {}", file_name);
                files_to_embed.push((file_id_str, full_path, file_name));
            }
        }
    }

    // Remove deleted files from index (smart eviction)
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

        // Process each file that needs embedding
        for (file_idx, (file_id_str, full_path, file_name)) in files_to_embed.iter().enumerate() {
            // Get content and HMAC again
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

            // Create chunks for this file
            let mut chunks: Vec<ChunkRecord> = Vec::new();
            let sections = split_markdown_sections(&content);
            
            for section in &sections {
                let parent_chunks = chunk_text(section, PARENT_CHARS, PARENT_OVERLAP, MAX_CHUNKS_PER_FILE);
                for parent in &parent_chunks {
                    let child_chunks = chunk_text(parent, CHILD_CHARS, CHILD_OVERLAP, MAX_CHUNKS_PER_FILE);
                    for child in child_chunks {
                        if child.trim().is_empty() { continue; }
                        chunks.push(ChunkRecord {
                            file_id: file_id_str.clone(),
                            file_path: full_path.clone(),
                            file_name: file_name.clone(),
                            parent_chunk: parent.clone(),
                            child_text: child,
                        });
                    }
                }
            }

            let file_intro: String = content.chars().take(200).collect();
            chunks.push(ChunkRecord {
                file_id: file_id_str.clone(),
                file_path: full_path.clone(),
                file_name: file_name.clone(),
                parent_chunk: format!("{}\n{}", file_name, file_intro),
                child_text: file_name.clone(),
            });

            // Embed all chunks for this file - fail the whole file if any batch fails
            let mut all_vectors: Vec<f32> = Vec::new();
            let mut failed = false;
            
            for batch in chunks.chunks(BATCH_SIZE) {
                let texts: Vec<String> = batch.iter().map(|c| c.child_text.clone()).collect();
                match embed_batch(&mut bi_session, &bi_tokenizer, &texts, BI_HIDDEN, BI_MAX_LEN) {
                    Ok(vecs) => {
                        for vec in vecs {
                            all_vectors.extend_from_slice(&vec);
                        }
                    }
                    Err(e) => {
                        eprintln!("\nError: batch embed failed for file {}: {:?}", file_name, e);
                        failed = true;
                        break;
                    }
                }
            }
            
            // Only store if all batches succeeded
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

        // Save index
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

    println!("🔎 Searching...");
    let query_vec = embed_single(&mut bi_session, &bi_tokenizer, query, "query", BI_HIDDEN, BI_MAX_LEN)?;

    // Collect all chunks and vectors from all files
    let mut all_chunks: Vec<ChunkRecord> = Vec::new();
    let mut all_vectors: Vec<f32> = Vec::new();
    
    for file_index in search_index.files.values() {
        for chunk in &file_index.chunks {
            all_chunks.push(chunk.clone());
        }
        all_vectors.extend_from_slice(&file_index.vectors);
    }

    let num_chunks = all_chunks.len();
    if num_chunks == 0 {
        println!("No chunks indexed yet.");
        return Ok(());
    }

    // Calculate similarities
    let mut scores: Vec<(usize, f32)> = (0..num_chunks)
        .map(|i| {
            let base = i * BI_HIDDEN;
            let sim: f32 = (0..BI_HIDDEN).map(|j| query_vec[j] * all_vectors[base + j]).sum();
            (i, sim)
        })
        .collect();

    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let candidates: Vec<(usize, f32)> = scores.into_iter().take(RERANK_K).collect();

    println!("  ✓ Reranking {} candidates...", candidates.len());
    let rerank_start = std::time::Instant::now();
    let mut reranked: Vec<(String, String, f32)> = Vec::new();

    for (idx, _) in &candidates {
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

    let mut seen: HashSet<String> = HashSet::new();
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
            let sig = 1.0f32 / (1.0 + (-score).exp());
            let score_percent = (sig * 100.0).clamp(0.0, 100.0) as usize;
            let bar_len = score_percent / 2;
            let bar = "█".repeat(bar_len);
            let empty = "░".repeat(50usize.saturating_sub(bar_len));
            let score_colored = if sig > 0.7 { format!("{:.4}", sig).green() }
                                else if sig > 0.4 { format!("{:.4}", sig).yellow() }
                                else { format!("{:.4}", sig).red() };
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
    let (_, logits) = outputs["logits"].try_extract_tensor::<f32>()
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

    let mut all_ids  = vec![0i64; batch_size * max_len];
    let mut all_mask = vec![0i64; batch_size * max_len];
    let mut lengths  = vec![0usize; batch_size];

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