use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use lb_rs::model::core_config::Config;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::Lb;
use lb_rs::Uuid;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SYNC_FOLDER: &str = "/bifs";
pub const DATA_DIR: &str = ".lb-bifs";
pub const INDEX_FILE: &str = "index.json";
pub const BASE_DIR: &str = "base";

pub fn compute_hash(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

pub type Hmac = [u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: Uuid,
    pub path: String,
    pub hash: String,
    pub hmac: Option<Hmac>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Index {
    pub files: HashMap<Uuid, FileRecord>,
}

impl Index {
    pub fn load(data_dir: &PathBuf) -> Self {
        let path = data_dir.join(INDEX_FILE);
        if path.exists() {
            let content = fs::read_to_string(&path).unwrap();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self, data_dir: &PathBuf) {
        let path = data_dir.join(INDEX_FILE);
        let content = serde_json::to_string_pretty(self).unwrap();
        fs::write(path, content).unwrap();
    }
}

pub struct BiFS {
    pub lb: Lb,
    pub root: PathBuf,
    pub data_dir: PathBuf,
    pub index: Index,
}

impl BiFS {
    pub fn new(lb: Lb, root: PathBuf) -> Self {
        let data_dir = root.join(DATA_DIR);
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(data_dir.join(BASE_DIR)).unwrap();
        let index = Index::load(&data_dir);
        Self { lb, root, data_dir, index }
    }

    pub async fn init() -> Self {
        let lb = Lb::init(Config::cli_config("cli")).await.unwrap();
        let root = env::current_dir().unwrap();
        Self::new(lb, root)
    }

    fn save_base(&self, hash: &str, content: &[u8]) {
        let path = self.data_dir.join(BASE_DIR).join(hash);
        fs::write(path, content).unwrap();
    }

    fn read_base(&self, hash: &str) -> Vec<u8> {
        let path = self.data_dir.join(BASE_DIR).join(hash);
        fs::read(path).unwrap()
    }

    fn delete_base(&self, hash: &str) {
        let path = self.data_dir.join(BASE_DIR).join(hash);
        let _ = fs::remove_file(path);
    }

    fn three_way_merge(&self, base: &[u8], local: &[u8], remote: &[u8]) -> Vec<u8> {
        let base = String::from_utf8_lossy(base).to_string();
        let local = String::from_utf8_lossy(local).to_string();
        let remote = String::from_utf8_lossy(remote).to_string();

        Buffer::from(base.as_str())
            .merge(local, remote)
            .into_bytes()
    }

    async fn get_or_create_sync_root(&self) -> lb_rs::model::file::File {
        match self.lb.get_by_path(SYNC_FOLDER).await {
            Ok(f) => f,
            Err(_) => {
                let f = self.lb.create_at_path(&format!("{}/", SYNC_FOLDER)).await.unwrap();
                self.lb.sync().await.unwrap();
                f
            }
        }
    }

    /// Returns true if the document was processed, false if skipped (e.g., file relocated)
    fn pull_document(&self, old: Option<&FileRecord>, new: &FileRecord, remote_content: &[u8]) -> bool {
        let local_path = self.root.join(&new.path);

        let final_content = match old {
            Some(old_record) => {
                // file was previously pulled - check if it still exists at expected path
                if !local_path.exists() {
                    // file was relocated, skip and let push handle it
                    return false;
                }

                let local_content = fs::read(&local_path).unwrap();
                let local_hash = compute_hash(&local_content);

                let content = if local_hash == old_record.hash {
                    // local unchanged, just overwrite
                    remote_content.to_vec()
                } else {
                    // local changed, 3-way merge
                    let base = self.read_base(&old_record.hash);
                    self.three_way_merge(&base, &local_content, remote_content)
                };

                self.delete_base(&old_record.hash);
                content
            }
            None => {
                // new file, create parent dirs
                if let Some(parent) = local_path.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                remote_content.to_vec()
            }
        };

        fs::write(&local_path, &final_content).unwrap();
        self.save_base(&new.hash, remote_content);

        println!("{}", new.path);
        true
    }

    async fn push_document(&mut self, _old: Option<&FileRecord>, _new: &FileRecord) {
        todo!()
    }

    pub async fn pull(&mut self) {
        self.lb.sync().await.unwrap();

        let sync_root = self.get_or_create_sync_root().await;
        let files = self.lb.get_and_get_children_recursively(&sync_root.id).await.unwrap();

        // collect IDs of documents currently in lockbook
        let remote_ids: std::collections::HashSet<_> = files
            .iter()
            .filter(|f| f.is_document())
            .map(|f| f.id)
            .collect();

        // handle deletions: files in our index but not in lockbook
        let deleted_ids: Vec<_> = self.index.files.keys()
            .filter(|id| !remote_ids.contains(id))
            .copied()
            .collect();

        for id in deleted_ids {
            if let Some(record) = self.index.files.remove(&id) {
                let local_path = self.root.join(&record.path);
                let _ = fs::remove_file(&local_path);
                self.delete_base(&record.hash);
                println!("deleted: {}", record.path);
            }
        }

        // handle updates and new files
        for file in files {
            if !file.is_document() {
                continue;
            }

            let lb_path = self.lb.get_path_by_id(file.id).await.unwrap();
            let relative_path = lb_path.strip_prefix(SYNC_FOLDER).unwrap().trim_start_matches('/');
            let (hmac, content) = self.lb.read_document_with_hmac(file.id, false).await.unwrap();
            let hash = compute_hash(&content);

            let old = self.index.files.get(&file.id);

            // check if file was relocated in lockbook
            if let Some(old_record) = old {
                if old_record.path != relative_path {
                    // file relocated in lockbook, move local file
                    let old_path = self.root.join(&old_record.path);
                    let new_path = self.root.join(relative_path);

                    if old_path.exists() {
                        if let Some(parent) = new_path.parent() {
                            fs::create_dir_all(parent).unwrap();
                        }
                        fs::rename(&old_path, &new_path).unwrap();
                        println!("moved: {} -> {}", old_record.path, relative_path);
                    }
                }
            }

            let new = FileRecord {
                id: file.id,
                path: relative_path.to_string(),
                hash,
                hmac,
            };

            if self.pull_document(old, &new, &content) {
                self.index.files.insert(file.id, new);
            }
        }

        self.index.save(&self.data_dir);
    }

    pub async fn push(&mut self) {
        let records: Vec<_> = self.index.files.values().cloned().collect();
        for record in records {
            self.push_document(Some(&record), &record).await;
        }

        self.lb.sync().await.unwrap();
        self.index.save(&self.data_dir);
    }
}
