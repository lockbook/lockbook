use crate::{logic::{crypto::EncryptedDocument, SharedErrorKind, SharedResult}, model::{core_config::Config, file_metadata::DocumentHmac}};
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct AsyncDocs {
    location: PathBuf,
}

impl AsyncDocs {
    pub async fn insert(
        &self, id: Uuid, hmac: Option<DocumentHmac>, document: &EncryptedDocument,
    ) -> SharedResult<()> {
        if let Some(hmac) = hmac {
            let value = &bincode::serialize(document)?;
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

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get(
        &self, id: Uuid, hmac: Option<DocumentHmac>,
    ) -> SharedResult<EncryptedDocument> {
        self.maybe_get(id, hmac)
            .await?
            .ok_or_else(|| SharedErrorKind::FileNonexistent.into())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn maybe_get(
        &self, id: Uuid, hmac: Option<DocumentHmac>,
    ) -> SharedResult<Option<EncryptedDocument>> {
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
                Some(data) => bincode::deserialize(&data).map(Some)?,
                None => None,
            })
        } else {
            Ok(None)
        }
    }

    pub async fn delete(&self, id: Uuid, hmac: Option<DocumentHmac>) -> SharedResult<()> {
        if let Some(hmac) = hmac {
            let path_str = key_path(&self.location, id, hmac);
            let path = Path::new(&path_str);
            trace!("delete\t{}", &path_str);
            if path.exists() {
                fs::remove_file(path).await?;
            }
        }

        Ok(())
    }
}

pub fn namespace_path(writeable_path: &PathBuf) -> String {
    format!("{}/documents", writeable_path.to_str().unwrap())
}

pub fn key_path(writeable_path: &PathBuf, key: Uuid, hmac: DocumentHmac) -> String {
    let hmac = base64::encode_config(hmac, base64::URL_SAFE);
    format!("{}/{}-{}", namespace_path(writeable_path), key, hmac)
}

impl From<&Config> for AsyncDocs {
    fn from(cfg: &Config) -> Self {
        Self { location: PathBuf::from(&cfg.writeable_path) }
    }
}
