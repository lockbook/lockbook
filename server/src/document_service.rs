use crate::ServerError;
use crate::config::Config;
use lb_rs::model::crypto::EncryptedDocument;
use lb_rs::model::doc_info::BlobKey;
use lb_rs::model::errors::{LbErr, LbErrKind, LbResult};
use std::fmt::Debug;
use std::future::Future;
use std::path::PathBuf;
use tokio::fs::{File, remove_file};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub trait DocumentService: Clone + Send + Sync + 'static {
    fn insert(
        &self, key: &BlobKey, content: &EncryptedDocument,
    ) -> impl Future<Output = LbResult<()>> + Send;

    fn maybe_get(
        &self, key: &BlobKey,
    ) -> impl Future<Output = LbResult<Option<EncryptedDocument>>> + Send;

    fn get(&self, key: &BlobKey) -> impl Future<Output = LbResult<EncryptedDocument>> + Send {
        async move {
            self.maybe_get(key)
                .await?
                .ok_or_else(|| LbErrKind::FileNonexistent.into())
        }
    }

    fn delete(&self, key: &BlobKey) -> impl Future<Output = LbResult<()>> + Send;

    fn exists(&self, key: &BlobKey) -> bool;

    fn get_path(&self, key: &BlobKey) -> PathBuf;
}

/// A store failure as an endpoint's internal error.
pub fn stored<T: Debug>(err: LbErr) -> ServerError<T> {
    internal!("document store: {:?}", err)
}

#[derive(Clone)]
pub struct OnDiskDocuments {
    config: Config,
}

impl From<&Config> for OnDiskDocuments {
    fn from(value: &Config) -> Self {
        Self { config: value.clone() }
    }
}

fn unexpected(err: impl Debug) -> LbErr {
    LbErrKind::Unexpected(format!("{err:?}")).into()
}

impl DocumentService for OnDiskDocuments {
    async fn insert(&self, key: &BlobKey, content: &EncryptedDocument) -> LbResult<()> {
        let content = bincode::serialize(content).map_err(unexpected)?;
        let path = self.get_path(key);
        let mut file = File::create(path).await.map_err(unexpected)?;
        file.write_all(&content).await.map_err(unexpected)?;
        file.flush().await.map_err(unexpected)?;
        Ok(())
    }

    async fn maybe_get(&self, key: &BlobKey) -> LbResult<Option<EncryptedDocument>> {
        let path = self.get_path(key);
        if !path.exists() {
            return Ok(None);
        }
        let mut file = File::open(path).await.map_err(unexpected)?;
        let mut content = vec![];
        file.read_to_end(&mut content).await.map_err(unexpected)?;
        Ok(Some(bincode::deserialize(&content).map_err(unexpected)?))
    }

    async fn delete(&self, key: &BlobKey) -> LbResult<()> {
        let path = self.get_path(key);
        // I'm not sure this check should exist, the two situations it gets utilized is when we re-delete
        // an already deleted file and when we move a file from version 0 -> N. Maybe it would be more
        // efficient for the caller to look at the metadata and make a more informed decision about
        // whether this needs to be called or not. Perhaps an async version should be used if we do keep
        // the check.
        if path.exists() {
            remove_file(path).await.map_err(unexpected)?;
        }
        Ok(())
    }

    fn exists(&self, key: &BlobKey) -> bool {
        self.get_path(key).exists()
    }

    fn get_path(&self, key: &BlobKey) -> PathBuf {
        let mut path = self.config.files.path.clone();
        match key {
            BlobKey::Legacy { id, hmac } => {
                // we may need to truncate this
                let hmac = base64::encode_config(hmac, base64::URL_SAFE);
                path.push(format!("{id}-{hmac}"));
            }
            BlobKey::Version { id, version } => path.push(format!("{id}.{version}")),
        }
        path
    }
}
