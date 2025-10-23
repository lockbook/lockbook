use crate::ServerError;
use crate::config::Config;
use async_trait::async_trait;
use lb_rs::model::crypto::EncryptedDocument;
use lb_rs::model::file_metadata::DocumentHmac;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::fs::{File, remove_file};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[async_trait]
pub trait DocumentService: Send + Sync + Clone + 'static {
    async fn insert<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac, content: &EncryptedDocument,
    ) -> Result<(), ServerError<T>>;
    async fn get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<EncryptedDocument, ServerError<T>>;
    async fn maybe_get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<Option<EncryptedDocument>, ServerError<T>>;
    async fn delete<T: Debug>(&self, id: &Uuid, hmac: &DocumentHmac) -> Result<(), ServerError<T>>;

    fn exists(&self, id: &Uuid, hmac: &DocumentHmac) -> bool;
    fn get_path(&self, id: &Uuid, hmac: &DocumentHmac) -> PathBuf;
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
        &self, id: &Uuid, hmac: &DocumentHmac, content: &EncryptedDocument,
    ) -> Result<(), ServerError<T>> {
        let content = bincode::serialize(content)?;
        let path = self.get_path(id, hmac);
        let mut file = File::create(path.clone()).await?;
        file.write_all(&content)
            .await
            .map_err(|err| internal!("{:?}", err))?;
        file.flush().await.map_err(|err| internal!("{:?}", err))?;
        Ok(())
    }

    async fn get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<EncryptedDocument, ServerError<T>> {
        let path = self.get_path(id, hmac);
        let mut file = File::open(path.clone()).await?;
        let mut content = vec![];
        file.read_to_end(&mut content).await?;
        let content = bincode::deserialize(&content)?;
        Ok(content)
    }

    async fn maybe_get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<Option<EncryptedDocument>, ServerError<T>> {
        let path = self.get_path(id, hmac);
        if !path.exists() {
            return Ok(None);
        }

        Ok(Some(self.get(id, hmac).await?))
    }


    async fn delete<T: Debug>(&self, id: &Uuid, hmac: &DocumentHmac) -> Result<(), ServerError<T>> {
        let path = self.get_path(id, hmac);
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

    fn exists(&self, id: &Uuid, hmac: &DocumentHmac) -> bool {
        self.get_path(id, hmac).exists()
    }

    fn get_path(&self, id: &Uuid, hmac: &DocumentHmac) -> PathBuf {
        let mut path = self.config.files.path.clone();
        // we may need to truncate this
        let hmac = base64::encode_config(hmac, base64::URL_SAFE);
        path.push(format!("{id}-{hmac}"));
        path
    }
}

/// For use with fuzzer, not to be hooked up in prod
#[derive(Clone, Default)]
pub struct InMemDocuments {
    pub docs: Arc<Mutex<HashMap<String, EncryptedDocument>>>,
}

#[async_trait]
impl DocumentService for InMemDocuments {
    async fn insert<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac, content: &EncryptedDocument,
    ) -> Result<(), ServerError<T>> {
        let hmac = base64::encode_config(hmac, base64::URL_SAFE);
        let key = format!("{id}-{hmac}");
        self.docs.lock().unwrap().insert(key, content.clone());
        Ok(())
    }

    async fn get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<EncryptedDocument, ServerError<T>> {
        let hmac = base64::encode_config(hmac, base64::URL_SAFE);
        let key = format!("{id}-{hmac}");
        Ok(self.docs.lock().unwrap().get(&key).unwrap().clone())
    }

    async fn maybe_get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<Option<EncryptedDocument>, ServerError<T>> {
        let hmac = base64::encode_config(hmac, base64::URL_SAFE);
        let key = format!("{id}-{hmac}");
        Ok(self.docs.lock().unwrap().get(&key).cloned())
    }

    fn exists(&self, id: &Uuid, hmac: &DocumentHmac) -> bool {
        let hmac = base64::encode_config(hmac, base64::URL_SAFE);
        let key = format!("{id}-{hmac}");
        self.docs.lock().unwrap().contains_key(&key)
    }

    fn get_path(&self, _id: &Uuid, _hmac: &DocumentHmac) -> PathBuf {
        unimplemented!()
    }

    async fn delete<T: Debug>(&self, id: &Uuid, hmac: &DocumentHmac) -> Result<(), ServerError<T>> {
        let hmac = base64::encode_config(hmac, base64::URL_SAFE);
        let key = format!("{id}-{hmac}");
        self.docs.lock().unwrap().remove(&key);

        Ok(())
    }
}
