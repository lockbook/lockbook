use crate::model::errors::{LbErr, LbErrKind, LbResult, Unexpected, UnexpectedError};
use crate::model::file::File;
use crate::model::filename::DocumentType;
use crate::service::activity::RankingWeights;
use crate::service::events::Event;
use crate::Lb;
use futures::stream::{self, FuturesUnordered, StreamExt, TryStreamExt};
use serde::Serialize;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use sublime_fuzzy::{FuzzySearch, Scoring};
use tantivy::collector::TopDocs;
use tantivy::query::{QueryParser, RegexQuery};
use tantivy::schema::{Schema, STORED, TEXT};
use tantivy::{
    doc, Document, Index, IndexReader, IndexWriter, ReloadPolicy, SnippetGenerator, TantivyDocument,
};
use tokio::sync::RwLock;
use uuid::Uuid;

const CONTENT_MAX_LEN_BYTES: usize = 128 * 1024; // 128kb

#[derive(Clone)]
pub struct SearchIndex {
    pub ready: Arc<AtomicBool>,

    pub metadata_index: Arc<RwLock<SearchMetadata>>,
    pub tantivy_index: Index,
    pub tantivy_reader: IndexReader,
}

#[derive(Copy, Clone, Debug)]
pub enum SearchConfig {
    Paths,
    Documents,
    PathsAndDocuments,
    Advanced,
}

#[derive(Debug)]
pub enum SearchResult {
    DocumentMatch { id: Uuid, path: String, content_matches: Vec<ContentMatch> },
    PathMatch { id: Uuid, path: String, matched_indices: Vec<usize>, score: i64 },
}

impl Lb {
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<Vec<SearchResult>> {
        // show suggested docs if the input string is empty
        if input.is_empty() {
            return stream::iter(self.suggested_docs(RankingWeights::default()).await?)
                .then(|id| async move {
                    Ok(SearchResult::PathMatch {
                        id,
                        path: self.get_path_by_id(id).await?,
                        matched_indices: vec![],
                        score: 0,
                    })
                })
                .try_collect()
                .await;
        }

        let searcher = self.search.tantivy_reader.searcher();
        let schema = self.search.tantivy_index.schema();
        let title = schema.get_field("title").unwrap();
        let content = schema.get_field("content").unwrap();

        let query_parser = QueryParser::for_index(&self.search.tantivy_index, vec![title, content]);

        let query = query_parser.parse_query(input).map_unexpected()?;

        let mut snippet_generator =
            SnippetGenerator::create(&searcher, &query, content).map_unexpected()?;
        snippet_generator.set_max_num_chars(100);

        let title_snip = SnippetGenerator::create(&searcher, &query, title).map_unexpected()?;

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(10))
            .map_unexpected()?;

        let mut results = vec![];
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address).map_unexpected()?;
            for (field, _) in retrieved_doc.get_sorted_field_values() {
                if field == title {
                    let title = title_snip.snippet_from_doc(&retrieved_doc);
                    results.push(SearchResult::PathMatch {
                        id: Uuid::nil(), // todo
                        path: title.fragment().to_string(),
                        matched_indices: Self::highlight_to_matches(title.highlighted()),
                        score: 0,
                    });
                }

                if field == content {
                    let snippet = snippet_generator.snippet_from_doc(&retrieved_doc);
                    results.push(SearchResult::DocumentMatch {
                        id: Uuid::nil(),      // todo
                        path: "".to_string(), // todo
                        content_matches: vec![ContentMatch {
                            paragraph: snippet.fragment().to_string(),
                            matched_indices: Self::highlight_to_matches(snippet.highlighted()),
                            score: 0,
                        }],
                    });
                }
            }
        }

        results = self.search.metadata_index.read().await.path_search(input)?;

        Ok(results)
    }

    fn highlight_to_matches(ranges: &[Range<usize>]) -> Vec<usize> {
        let mut matches = vec![];
        for range in ranges {
            for i in range.clone() {
                matches.push(i);
            }
        }

        matches
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn build_index(&self) -> LbResult<()> {
        // if we haven't signed in yet, we'll leave our index entry and our event subscriber will
        // handle the state change
        if self.keychain.get_account().is_err() {
            return Ok(());
        }

        let schema = self.search.tantivy_index.schema();

        let mut index_writer: IndexWriter = self
            .search
            .tantivy_index
            .writer(50_000_000)
            .map_unexpected()?;

        let content = schema.get_field("content").unwrap();

        let metadata_index = SearchMetadata::populate(&self).await?;

        for file in &metadata_index.files {
            if !file.name.ends_with(".md") || file.is_folder() {
                continue;
            };
            let doc = String::from_utf8(self.read_document(file.id, false).await?).unwrap();

            if doc.len() > CONTENT_MAX_LEN_BYTES {
                continue;
            };
            index_writer
                .add_document(doc!(
                    content => doc,
                ))
                .map_unexpected()?;
        }

        index_writer.commit().map_unexpected()?;

        *self.search.metadata_index.write().await = metadata_index;

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub fn setup_search(&self) {
        if self.config.background_work {
            let lb = self.clone();
            let mut rx = self.subscribe();
            tokio::spawn(async move {
                lb.build_index().await.unwrap();
                loop {
                    let evt = match rx.recv().await {
                        Ok(evt) => evt,
                        Err(err) => {
                            error!("failed to receive from a channel {err}");
                            return;
                        }
                    };

                    match evt {
                        Event::MetadataChanged => {}
                        Event::DocumentWritten(id) => {}
                        _ => {}
                    };
                }
            });
        }
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("title", TEXT | STORED);
        schema_builder.add_text_field("content", TEXT | STORED);

        let schema = schema_builder.build();

        let index = Index::create_in_ram(schema.clone());
        //index.set_multithread_executor(10).unwrap();

        // this probably shouldn't happen here, see the doc for try_into()
        // but I think that doc is written for a disk reader so let's see
        // if it actually matters
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .unwrap();

        Self {
            ready: Default::default(),
            tantivy_index: index,
            tantivy_reader: reader,
            metadata_index: Default::default(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ContentMatch {
    pub paragraph: String,
    pub matched_indices: Vec<usize>,
    pub score: i64,
}

impl SearchResult {
    pub fn id(&self) -> Uuid {
        match self {
            SearchResult::DocumentMatch { id, .. } | SearchResult::PathMatch { id, .. } => *id,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            SearchResult::DocumentMatch { path, .. } | SearchResult::PathMatch { path, .. } => path,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            SearchResult::DocumentMatch { path, .. } | SearchResult::PathMatch { path, .. } => {
                path.split('/').last().unwrap_or_default()
            }
        }
    }

    pub fn score(&self) -> i64 {
        match self {
            SearchResult::DocumentMatch { content_matches, .. } => content_matches
                .iter()
                .map(|m| m.score)
                .max()
                .unwrap_or_default(),
            SearchResult::PathMatch { score, .. } => *score,
        }
    }
}

#[derive(Default)]
pub struct SearchMetadata {
    files: Vec<File>,
    paths: Vec<(Uuid, String)>,
    suggested_docs: Vec<Uuid>,
}

impl SearchMetadata {
    async fn populate(lb: &Lb) -> LbResult<Self> {
        let files = lb.list_metadatas().await?;
        let paths = lb.list_paths_with_ids(None).await?;
        let suggested_docs = lb.suggested_docs(RankingWeights::default()).await?;

        Ok(SearchMetadata { files, paths, suggested_docs })
    }

    fn path_search(&self, query: &str) -> LbResult<Vec<SearchResult>> {
        let mut results = self.path_candidates(query)?;

        self.apply_score(&mut results);
        results.sort_by_key(|r| -r.score());

        Ok(results)
    }

    fn path_candidates(&self, query: &str) -> LbResult<Vec<SearchResult>> {
        let mut search_results = vec![];

        for (id, path) in &self.paths {
            let mut matched_indices = vec![];

            let mut query_iter = query.chars().rev();
            let mut current_query_char = query_iter.next();

            for (path_ind, path_char) in path.char_indices().rev() {
                if let Some(qc) = current_query_char {
                    if qc == path_char {
                        matched_indices.push(path_ind);
                        current_query_char = query_iter.next();
                    }
                } else {
                    break;
                }
            }

            if current_query_char.is_none() {
                search_results.push(SearchResult::PathMatch {
                    id: *id,
                    path: path.clone(),
                    matched_indices,
                    score: 0,
                });
            }
        }
        Ok(search_results)
    }

    fn apply_score(&self, candidates: &mut [SearchResult]) {
        let size = 10;
        let suggested = 10;
        let title = 30;
        let editable = 3;

        candidates.sort_by_key(|a| a.path().len());

        // the 10 smallest paths start with a mild advantage
        for i in 0..size {
            if let Some(cand) = candidates.get_mut(i) {
                if let SearchResult::PathMatch { id: _, path, matched_indices: _, score } = cand {
                    *score = (10 - i) as i64;
                    println!("smallest 10 {path} {score}");
                }
            }
        }

        // items in suggested docs have their score boosted
        for cand in candidates.iter_mut() {
            if self.suggested_docs.contains(&cand.id()) {
                if let SearchResult::PathMatch { id: _, path, matched_indices: _, score } = cand {
                    *score += suggested;
                    println!("suggested {path} {score}");
                }
            }
        }

        // to what extent is the match in the name of the file
        for cand in candidates.iter_mut() {
            if let SearchResult::PathMatch { id: _, path, matched_indices, score } = cand {
                let mut name_match = 0;
                let mut name_size = 0;

                for (i, c) in path.char_indices().rev() {
                    if c == '/' {
                        break;
                    }
                    name_size += 1;
                    if matched_indices.contains(&i) {
                        name_match += 1;
                    }
                }

                let match_portion = name_match as f32 / name_size.max(1) as f32;
                *score += (match_portion * title as f32) as i64;
                println!("title {path} {score}");
            }
        }

        // if this document is editable in platform
        for cand in candidates.iter_mut() {
            if let SearchResult::PathMatch { id: _, path, matched_indices: _, score } = cand {
                if path.ends_with(".md") || path.ends_with(".svg") {
                    *score += editable;
                    println!("editable {path} {score}");
                }
            }
        }
    }
}
