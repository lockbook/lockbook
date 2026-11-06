use crate::ServerError;
use crate::config::Config;
use async_trait::async_trait;
use lb_rs::model::crypto::EncryptedDocument;
use lb_rs::model::doc_info::BlobKey;
use std::fmt::Debug;
use std::path::PathBuf;
use tokio::fs::{File, remove_file};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[async_trait]
pub trait DocumentService: Send + Sync + Clone + 'static {
    async fn insert<T: Debug>(
        &self, key: &BlobKey, content: &EncryptedDocument,
    ) -> Result<(), ServerError<T>>;
    async fn get<T: Debug>(&self, key: &BlobKey) -> Result<EncryptedDocument, ServerError<T>>;
    async fn maybe_get<T: Debug>(
        &self, key: &BlobKey,
    ) -> Result<Option<EncryptedDocument>, ServerError<T>>;
    async fn delete<T: Debug>(&self, key: &BlobKey) -> Result<(), ServerError<T>>;

    fn exists(&self, key: &BlobKey) -> bool;
    fn get_path(&self, key: &BlobKey) -> PathBuf;
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

#[async_trait]
impl DocumentService for OnDiskDocuments {
    async fn insert<T: Debug>(
        &self, key: &BlobKey, content: &EncryptedDocument,
    ) -> Result<(), ServerError<T>> {
        let content = bincode::serialize(content)?;
        let path = self.get_path(key);
        let mut file = File::create(path.clone()).await?;
        file.write_all(&content)
            .await
            .map_err(|err| internal!("{:?}", err))?;
        file.flush().await.map_err(|err| internal!("{:?}", err))?;
        Ok(())
    }

    async fn get<T: Debug>(&self, key: &BlobKey) -> Result<EncryptedDocument, ServerError<T>> {
        let path = self.get_path(key);
        let mut file = File::open(path.clone()).await?;
        let mut content = vec![];
        file.read_to_end(&mut content).await?;
        let content = bincode::deserialize(&content)?;
        Ok(content)
    }

    async fn maybe_get<T: Debug>(
        &self, key: &BlobKey,
    ) -> Result<Option<EncryptedDocument>, ServerError<T>> {
        let path = self.get_path(key);
        if !path.exists() {
            return Ok(None);
        }

        Ok(Some(self.get(key).await?))
    }

    async fn delete<T: Debug>(&self, key: &BlobKey) -> Result<(), ServerError<T>> {
        let path = self.get_path(key);
        // I'm not sure this check should exist, the two situations it gets utilized is when we re-delete
        // an already deleted file and when we move a file from version 0 -> N. Maybe it would be more
        // efficient for the caller to look at the metadata and make a more informed decision about
        // whether this needs to be called or not. Perhaps an async version should be used if we do keep
        // the check.
        if path.exists() {
            remove_file(path).await?;
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
