use crate::model::{
    core_config::Config,
    crypto::EncryptedDocument,
    errors::{LbErrKind, LbResult},
    file_metadata::DocumentHmac,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[cfg(not(target_family = "wasm"))]
use {
    crate::model::errors::Unexpected,
    std::io::ErrorKind,
    tokio::{
        fs::{self, File, OpenOptions},
        io::{AsyncReadExt, AsyncWriteExt},
    },
};

/// Where documents live: files under a data directory, or a map no other process can see.
#[derive(Clone)]
#[cfg_attr(target_family = "wasm", allow(dead_code))]
pub enum AsyncDocs {
    Disk { location: PathBuf },
    Memory(DocMap),
}

type DocMap = Arc<Mutex<HashMap<(Uuid, DocumentHmac), EncryptedDocument>>>;

impl AsyncDocs {
    pub fn in_memory() -> Self {
        Self::Memory(Default::default())
    }
}

/// One write, readable once [PendingDocument::promote]d into the store that wrote it. On disk
/// each write gets its own pending path, so writers of the same id and hmac never share one; in
/// memory the handle holds the document itself.
#[must_use]
#[cfg(not(target_family = "wasm"))]
pub enum PendingDocument {
    File { path: String, final_path: String },
    Memory { docs: DocMap, key: (Uuid, DocumentHmac), document: EncryptedDocument },
}

#[cfg(not(target_family = "wasm"))]
impl PendingDocument {
    pub async fn promote(self) -> LbResult<()> {
        match self {
            Self::File { path, final_path } => Ok(fs::rename(&path, &final_path).await?),
            Self::Memory { docs, key, document } => {
                docs.lock().unwrap().insert(key, document);
                Ok(())
            }
        }
    }
}

#[must_use]
#[cfg(target_family = "wasm")]
pub struct PendingDocument;

#[cfg(target_family = "wasm")]
impl PendingDocument {
    pub async fn promote(self) -> LbResult<()> {
        Ok(())
    }
}

#[cfg(target_family = "wasm")]
impl AsyncDocs {
    pub async fn insert(
        &self, _id: Uuid, _hmac: Option<DocumentHmac>, _document: &EncryptedDocument,
    ) -> LbResult<()> {
        Ok(())
    }

    pub async fn insert_pending(
        &self, _id: Uuid, _hmac: DocumentHmac, _document: &EncryptedDocument,
    ) -> LbResult<PendingDocument> {
        Ok(PendingDocument)
    }

    pub async fn get(&self, _id: Uuid, _hmac: Option<DocumentHmac>) -> LbResult<EncryptedDocument> {
        Err(LbErrKind::FileNonexistent.into())
    }

    pub async fn maybe_get(
        &self, _id: Uuid, _hmac: Option<DocumentHmac>,
    ) -> LbResult<Option<EncryptedDocument>> {
        Ok(None)
    }

    pub async fn delete(&self, _id: Uuid, _hmac: Option<DocumentHmac>) -> LbResult<()> {
        Ok(())
    }

    pub fn exists(&self, id: Uuid, hmac: Option<DocumentHmac>) -> bool {
        true
    }

    pub(crate) async fn retain(&self, _file_hmacs: HashSet<(Uuid, [u8; 32])>) -> LbResult<()> {
        Ok(())
    }
}

#[cfg(not(target_family = "wasm"))]
impl AsyncDocs {
    pub async fn insert(
        &self, id: Uuid, hmac: Option<DocumentHmac>, document: &EncryptedDocument,
    ) -> LbResult<()> {
        if let Some(hmac) = hmac {
            let pending = self.insert_pending(id, hmac, document).await?;
            pending.promote().await?;
        }
        Ok(())
    }

    pub async fn insert_pending(
        &self, id: Uuid, hmac: DocumentHmac, document: &EncryptedDocument,
    ) -> LbResult<PendingDocument> {
        let location = match self {
            Self::Memory(docs) => {
                let (docs, key, document) = (docs.clone(), (id, hmac), document.clone());
                return Ok(PendingDocument::Memory { docs, key, document });
            }
            Self::Disk { location } => location,
        };
        let value = &bincode::serialize(document).map_unexpected()?;
        let path_str = pending_path(location, id, hmac);
        let path = Path::new(&path_str);
        trace!("write pending\t{} {:?} bytes", &path_str, value.len());
        fs::create_dir_all(path.parent().unwrap()).await?;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .await?;
        f.write_all(value).await?;
        // tokio finishes the write on a blocking thread; wait so promote renames a complete file
        f.flush().await?;
        let final_path = key_path(location, id, hmac);
        Ok(PendingDocument::File { path: path_str, final_path })
    }

    pub fn exists(&self, id: Uuid, hmac: Option<DocumentHmac>) -> bool {
        let Some(hmac) = hmac else {
            return false; // is this the move?
        };
        match self {
            Self::Memory(docs) => docs.lock().unwrap().contains_key(&(id, hmac)),
            Self::Disk { location } => Path::new(&key_path(location, id, hmac)).exists(),
        }
    }

    pub async fn get(&self, id: Uuid, hmac: Option<DocumentHmac>) -> LbResult<EncryptedDocument> {
        self.maybe_get(id, hmac)
            .await?
            .ok_or_else(|| LbErrKind::FileNonexistent.into())
    }

    pub async fn maybe_get(
        &self, id: Uuid, hmac: Option<DocumentHmac>,
    ) -> LbResult<Option<EncryptedDocument>> {
        let Some(hmac) = hmac else {
            return Ok(None);
        };
        let location = match self {
            Self::Memory(docs) => return Ok(docs.lock().unwrap().get(&(id, hmac)).cloned()),
            Self::Disk { location } => location,
        };
        let path_str = key_path(location, id, hmac);
        let path = Path::new(&path_str);
        trace!("read\t{}", &path_str);
        let maybe_data: Option<Vec<u8>> = match File::open(path).await {
            Ok(mut f) => {
                let mut buffer: Vec<u8> = Vec::new();
                f.read_to_end(&mut buffer).await?;
                Some(buffer)
            }
            Err(err) => match err.kind() {
                ErrorKind::NotFound => None,
                _ => return Err(err.into()),
            },
        };

        Ok(match maybe_data {
            Some(data) => bincode::deserialize(&data).map(Some).map_unexpected()?,
            None => None,
        })
    }

    pub async fn maybe_size(&self, id: Uuid, hmac: Option<DocumentHmac>) -> LbResult<Option<u64>> {
        let Some(hmac) = hmac else {
            return Ok(None);
        };
        Ok(match self {
            Self::Memory(docs) => docs
                .lock()
                .unwrap()
                .get(&(id, hmac))
                .map(|d| bincode::serialized_size(d).unwrap_or_default()),
            Self::Disk { location } => Path::new(&key_path(location, id, hmac))
                .metadata()
                .ok()
                .map(|meta| meta.len()),
        })
    }

    pub async fn delete(&self, id: Uuid, hmac: Option<DocumentHmac>) -> LbResult<()> {
        let Some(hmac) = hmac else {
            return Ok(());
        };
        match self {
            Self::Memory(docs) => {
                docs.lock().unwrap().remove(&(id, hmac));
            }
            Self::Disk { location } => {
                let path_str = key_path(location, id, hmac);
                let path = Path::new(&path_str);
                trace!("delete\t{}", &path_str);
                if path.exists() {
                    fs::remove_file(path).await.map_unexpected()?;
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn retain(&self, file_hmacs: HashSet<(Uuid, [u8; 32])>) -> LbResult<()> {
        let location = match self {
            Self::Memory(docs) => {
                docs.lock()
                    .unwrap()
                    .retain(|key, _| file_hmacs.contains(key));
                return Ok(());
            }
            Self::Disk { location } => location,
        };
        let dir_path = namespace_path(location);
        fs::create_dir_all(&dir_path).await?;
        let mut entries = fs::read_dir(&dir_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(LbErrKind::Unexpected("could not get filename from os".to_string()))?;

            if file_name.contains("pending") {
                continue;
            }

            // files we did not write (e.g. .DS_Store) are left alone
            let Some((id, hmac)) = parse_key_path(file_name) else {
                continue;
            };

            if !file_hmacs.contains(&(id, hmac)) {
                self.delete(id, Some(hmac)).await?;
            }
        }
        Ok(())
    }
}

/// Inverse of [key_path]'s file name: `<uuid>-<base64 hmac>`.
fn parse_key_path(file_name: &str) -> Option<(Uuid, DocumentHmac)> {
    let id = Uuid::parse_str(file_name.get(..36)?).ok()?;
    let hmac = file_name.get(36..)?.strip_prefix('-')?;
    let hmac = base64::decode_config(hmac, base64::URL_SAFE).ok()?;
    Some((id, hmac.try_into().ok()?))
}

pub fn namespace_path(writeable_path: &Path) -> String {
    format!("{}/documents", writeable_path.to_str().unwrap())
}

pub fn key_path(writeable_path: &Path, key: Uuid, hmac: DocumentHmac) -> String {
    let hmac = base64::encode_config(hmac, base64::URL_SAFE);
    format!("{}/{}-{}", namespace_path(writeable_path), key, hmac)
}

fn pending_path(writeable_path: &Path, key: Uuid, hmac: DocumentHmac) -> String {
    format!("{}.{}.pending", key_path(writeable_path, key, hmac), Uuid::new_v4())
}

impl From<&Config> for AsyncDocs {
    fn from(cfg: &Config) -> Self {
        Self::Disk { location: PathBuf::from(&cfg.writeable_path) }
    }
}
