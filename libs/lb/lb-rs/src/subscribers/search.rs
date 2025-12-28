use crate::Lb;
use crate::model::errors::{LbResult, Unexpected};
use crate::model::file::File;
use crate::service::activity::RankingWeights;
use crate::service::events::Event;
use serde::Serialize;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{INDEXED, STORED, Schema, TEXT, Value};
use tantivy::snippet::SnippetGenerator;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term, doc};
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
}

#[derive(Debug)]
pub enum SearchResult {
    DocumentMatch { id: Uuid, path: String, content_matches: Vec<ContentMatch> },
    PathMatch { id: Uuid, path: String, matched_indices: Vec<usize>, score: i64 },
}

impl Lb {
    /// Lockbook's search implementation.
    ///
    /// Takes an input and a configuration. The configuration describes whether we are searching
    /// paths, documents or both.
    ///
    /// Document searches are handled by [tantivy](https://github.com/quickwit-oss/tantivy), and as
    /// such support [tantivy's advanced query
    /// syntax](https://docs.rs/tantivy/latest/tantivy/query/struct.QueryParser.html).
    /// In the future we plan to ingest a bunch of metadata and expose a full advanced search mode.
    ///
    /// Path searches are implemented as a subsequence filter with a number of hueristics to sort
    /// the results. Preference is given to shorter paths, filename matches, suggested docs, and
    /// documents that are editable in platform.
    ///
    /// Additionally if a path search contains a string, greater than 8 characters long that is
    /// contained within any of the paths in the search index, that result is returned with the
    /// highest score. lb:// style ids are also supported.
    #[instrument(level = "debug", skip(self, input), err(Debug))]
    pub async fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<Vec<SearchResult>> {
        // show suggested docs if the input string is empty
        if input.is_empty() {
            return self.search.metadata_index.read().await.empty_search();
        }

        match cfg {
            SearchConfig::Paths => {
                let mut results = self.search.metadata_index.read().await.path_search(input)?;
                results.truncate(5);
                Ok(results)
            }
            SearchConfig::Documents => {
                let mut results = self.search_content(input).await?;
                results.truncate(10);
                Ok(results)
            }
            SearchConfig::PathsAndDocuments => {
                let mut results = self.search.metadata_index.read().await.path_search(input)?;
                results.truncate(4);
                results.append(&mut self.search_content(input).await?);
                Ok(results)
            }
        }
    }

    async fn search_content(&self, input: &str) -> LbResult<Vec<SearchResult>> {
        let searcher = self.search.tantivy_reader.searcher();
        let schema = self.search.tantivy_index.schema();
        let id_field = schema.get_field("id").unwrap();
        let content = schema.get_field("content").unwrap();

        let query_parser = QueryParser::for_index(&self.search.tantivy_index, vec![content]);
        let mut results = vec![];

        if let Ok(query) = query_parser.parse_query(input) {
            let mut snippet_generator =
                SnippetGenerator::create(&searcher, &query, content).map_unexpected()?;
            snippet_generator.set_max_num_chars(100);

            let top_docs = searcher
                .search(&query, &TopDocs::with_limit(10))
                .map_unexpected()?;

            for (_score, doc_address) in top_docs {
                let retrieved_doc: TantivyDocument = searcher.doc(doc_address).map_unexpected()?;
                let id = Uuid::from_slice(
                    retrieved_doc
                        .get_first(id_field)
                        .map(|val| val.as_bytes().unwrap_or_default())
                        .unwrap_or_default(),
                )
                .map_unexpected()?;

                let snippet = snippet_generator.snippet_from_doc(&retrieved_doc);
                let path = self
                    .search
                    .metadata_index
                    .read()
                    .await
                    .paths
                    .iter()
                    .find(|(path_id, _)| *path_id == id)
                    .map(|(_, path)| path.to_string())
                    .unwrap_or_default();

                results.push(SearchResult::DocumentMatch {
                    id,
                    path,
                    content_matches: vec![ContentMatch {
                        paragraph: snippet.fragment().to_string(),
                        matched_indices: Self::highlight_to_matches(snippet.highlighted()),
                        score: 0,
                    }],
                });
            }
        }
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

        let new_metadata = SearchMetadata::populate(self).await?;

        let (deleted_ids, all_current_ids) = {
            let mut current_metadata = self.search.metadata_index.write().await;
            let deleted = new_metadata.compute_deleted(&current_metadata);
            let current = new_metadata.files.iter().map(|f| f.id).collect::<Vec<_>>();
            *current_metadata = new_metadata;
            (deleted, current)
        };

        self.update_tantivy(deleted_ids, all_current_ids).await;

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
                        Event::MetadataChanged => {
                            if let Some(replacement_index) =
                                SearchMetadata::populate(&lb).await.log_and_ignore()
                            {
                                let current_index = lb.search.metadata_index.read().await.clone();
                                let deleted_ids = replacement_index.compute_deleted(&current_index);
                                *lb.search.metadata_index.write().await = replacement_index;
                                lb.update_tantivy(deleted_ids, vec![]).await;
                            }
                        }
                        Event::DocumentWritten(id, _) => {
                            lb.update_tantivy(vec![id], vec![id]).await;
                        }
                        _ => {}
                    };
                }
            });
        }
    }

    async fn update_tantivy(&self, delete: Vec<Uuid>, add: Vec<Uuid>) {
        let mut index_writer: IndexWriter = self.search.tantivy_index.writer(50_000_000).unwrap();
        let schema = self.search.tantivy_index.schema();
        let id_field = schema.get_field("id").unwrap();
        let id_str = schema.get_field("id_str").unwrap();
        let content = schema.get_field("content").unwrap();

        for id in delete {
            let term = Term::from_field_bytes(id_field, id.as_bytes());
            index_writer.delete_term(term);
        }

        for id in add {
            let id_bytes = id.as_bytes().as_slice();
            let id_string = id.to_string();
            let Some(file) = self
                .search
                .metadata_index
                .read()
                .await
                .files
                .iter()
                .find(|f| f.id == id)
                .cloned()
            else {
                continue;
            };

            if !file.name.ends_with(".md") || file.is_folder() {
                continue;
            };

            let Ok(doc) = String::from_utf8(self.read_document(file.id, false).await.unwrap())
            else {
                continue;
            };

            if doc.len() > CONTENT_MAX_LEN_BYTES {
                continue;
            };

            index_writer
                .add_document(doc!(
                    id_field => id_bytes,
                    id_str => id_string,
                    content => doc,
                ))
                .unwrap();
        }

        index_writer.commit().unwrap();
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        let mut schema_builder = Schema::builder();
        schema_builder.add_bytes_field("id", INDEXED | STORED);
        schema_builder.add_text_field("id_str", TEXT | STORED);
        schema_builder.add_text_field("content", TEXT | STORED);

        let schema = schema_builder.build();

        let index = Index::create_in_ram(schema.clone());

        // doing this here would be a bad idea if not for in-ram empty index
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
                path.split('/').next_back().unwrap_or_default()
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

#[derive(Default, Clone)]
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

    fn compute_deleted(&self, old: &SearchMetadata) -> Vec<Uuid> {
        let mut deleted_ids = vec![];

        for old_file in &old.files {
            if !self.files.iter().any(|new_f| new_f.id == old_file.id) {
                deleted_ids.push(old_file.id);
            }
        }

        deleted_ids
    }

    fn empty_search(&self) -> LbResult<Vec<SearchResult>> {
        let mut results = vec![];

        for id in &self.suggested_docs {
            let path = self
                .paths
                .iter()
                .find(|(path_id, _)| id == path_id)
                .map(|(_, path)| path.clone())
                .unwrap_or_default();

            results.push(SearchResult::PathMatch {
                id: *id,
                path,
                matched_indices: vec![],
                score: 0,
            });
        }

        Ok(results)
    }

    fn path_search(&self, query: &str) -> LbResult<Vec<SearchResult>> {
        let mut results = self.path_candidates(query)?;
        self.score_paths(&mut results);

        results.sort_by_key(|r| -r.score());

        if let Some(result) = self.id_match(query) {
            results.insert(0, result);
        }

        Ok(results)
    }

    fn id_match(&self, query: &str) -> Option<SearchResult> {
        if query.len() < 8 {
            return None;
        }

        let query = if query.starts_with("lb://") {
            query.replacen("lb://", "", 1)
        } else {
            query.to_string()
        };

        for (id, path) in &self.paths {
            if id.to_string().contains(&query) {
                return Some(SearchResult::PathMatch {
                    id: *id,
                    path: path.clone(),
                    matched_indices: vec![],
                    score: 100,
                });
            }
        }

        None
    }

    fn path_candidates(&self, query: &str) -> LbResult<Vec<SearchResult>> {
        let mut search_results = vec![];

        for (id, path) in &self.paths {
            let mut matched_indices = vec![];

            let mut query_iter = query.chars().rev();
            let mut current_query_char = query_iter.next();

            for (path_ind, path_char) in path.char_indices().rev() {
                if let Some(qc) = current_query_char {
                    if qc.eq_ignore_ascii_case(&path_char) {
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

    fn score_paths(&self, candidates: &mut [SearchResult]) {
        // tunable bonuses for path search
        let smaller_paths = 10;
        let suggested = 10;
        let filename = 30;
        let editable = 3;

        candidates.sort_by_key(|a| a.path().len());

        // the 10 smallest paths start with a mild advantage
        for i in 0..smaller_paths {
            if let Some(SearchResult::PathMatch { id: _, path: _, matched_indices: _, score }) =
                candidates.get_mut(i)
            {
                *score = (smaller_paths - i) as i64;
            }
        }

        // items in suggested docs have their score boosted
        for cand in candidates.iter_mut() {
            if self.suggested_docs.contains(&cand.id()) {
                if let SearchResult::PathMatch { id: _, path: _, matched_indices: _, score } = cand
                {
                    *score += suggested;
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
                *score += (match_portion * filename as f32) as i64;
            }
        }

        // if this document is editable in platform
        for cand in candidates.iter_mut() {
            if let SearchResult::PathMatch { id: _, path, matched_indices: _, score } = cand {
                if path.ends_with(".md") || path.ends_with(".svg") {
                    *score += editable;
                }
            }
        }
    }
}
