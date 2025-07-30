use crate::model::core_config::Config;
use crate::model::crypto::EncryptedDocument;
use crate::model::errors::{LbErrKind, LbResult, Unexpected};
use crate::model::file_metadata::DocumentHmac;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[derive(Clone)]
pub struct AsyncDocs {
    pub(crate) dont_delete: Arc<AtomicBool>,
    location: PathBuf,
}

impl AsyncDocs {
    pub async fn insert(
        &self, id: Uuid, hmac: Option<DocumentHmac>, document: &EncryptedDocument,
    ) -> LbResult<()> {
        if let Some(hmac) = hmac {
            let value = &bincode::serialize(document).map_unexpected()?;
            let path_str = key_path(&self.location, id, hmac) + ".pending";
            let path = Path::new(&path_str);
            trace!("write\t{} {:?} bytes", &path_str, value.len());
            fs::create_dir_all(path.parent().unwrap()).await?;
            let mut f = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)
                .await?;
            f.write_all(value).await?;
            Ok(fs::rename(path, key_path(&self.location, id, hmac)).await?)
        } else {
            Ok(())
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
        if let Some(hmac) = hmac {
            let path_str = key_path(&self.location, id, hmac);
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
        } else {
            Ok(None)
        }
    }

    pub async fn maybe_size(&self, id: Uuid, hmac: Option<DocumentHmac>) -> LbResult<Option<u64>> {
        match hmac {
            Some(hmac) => {
                let path_str = key_path(&self.location, id, hmac);
                let path = Path::new(&path_str);
                Ok(path.metadata().ok().map(|meta| meta.len()))
            }
            None => Ok(None),
        }
    }

    pub async fn delete(&self, id: Uuid, hmac: Option<DocumentHmac>) -> LbResult<()> {
        if let Some(hmac) = hmac {
            let path_str = key_path(&self.location, id, hmac);
            let path = Path::new(&path_str);
            trace!("delete\t{}", &path_str);
            if path.exists() {
                fs::remove_file(path).await.map_unexpected()?;
            }
        }

        Ok(())
    }

    pub(crate) async fn retain(&self, file_hmacs: HashSet<(Uuid, [u8; 32])>) -> LbResult<()> {
        let dir_path = namespace_path(&self.location);
        fs::create_dir_all(&dir_path).await?;
        let mut entries = fs::read_dir(&dir_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(LbErrKind::Unexpected("could not get filename from os".to_string()))?;

            let (id_str, hmac_str) = file_name.split_at(36); // UUIDs are 36 characters long in string form

            let id = Uuid::parse_str(id_str).map_err(|err| {
                LbErrKind::Unexpected(format!("could not parse doc name as uuid {err:?}"))
            })?;

            let hmac_base64 = hmac_str
                .strip_prefix('-')
                .ok_or(LbErrKind::Unexpected("doc name missing -".to_string()))?;

            let hmac_bytes =
                base64::decode_config(hmac_base64, base64::URL_SAFE).map_err(|err| {
                    LbErrKind::Unexpected(format!("document disk file name malformed: {err:?}"))
                })?;

            let hmac: DocumentHmac = hmac_bytes.try_into().map_err(|err| {
                LbErrKind::Unexpected(format!("document disk file name malformed {err:?}"))
            })?;

            if !file_hmacs.contains(&(id, hmac)) {
                self.delete(id, Some(hmac)).await?;
            }
        }
        Ok(())
    }
}

pub fn namespace_path(writeable_path: &Path) -> String {
    format!("{}/documents", writeable_path.to_str().unwrap())
}

pub fn key_path(writeable_path: &Path, key: Uuid, hmac: DocumentHmac) -> String {
    let hmac = base64::encode_config(hmac, base64::URL_SAFE);
    format!("{}/{}-{}", namespace_path(writeable_path), key, hmac)
}

impl From<&Config> for AsyncDocs {
    fn from(cfg: &Config) -> Self {
        Self { location: PathBuf::from(&cfg.writeable_path), dont_delete: Default::default() }
    }
}
