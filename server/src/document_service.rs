use crate::ServerError;
use crate::config::Config;
use lb_rs::io::docs::DocumentStore;
use lb_rs::model::crypto::EncryptedDocument;
use lb_rs::model::errors::{LbErr, LbErrKind, LbResult};
use lb_rs::model::file_metadata::DocumentHmac;
use std::fmt::Debug;
use std::path::PathBuf;
use tokio::fs::{File, remove_file};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// The server's [DocumentStore], whose documents have paths the garbage worker reports.
pub trait DocumentService: DocumentStore {
    fn get_path(&self, id: Uuid, hmac: DocumentHmac) -> PathBuf;
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

impl DocumentStore for OnDiskDocuments {
    async fn insert(
        &self, id: Uuid, hmac: DocumentHmac, content: &EncryptedDocument,
    ) -> LbResult<()> {
        let content = bincode::serialize(content).map_err(unexpected)?;
        let path = self.get_path(id, hmac);
        let mut file = File::create(path).await.map_err(unexpected)?;
        file.write_all(&content).await.map_err(unexpected)?;
        file.flush().await.map_err(unexpected)?;
        Ok(())
    }

    async fn maybe_get(&self, id: Uuid, hmac: DocumentHmac) -> LbResult<Option<EncryptedDocument>> {
        let path = self.get_path(id, hmac);
        if !path.exists() {
            return Ok(None);
        }
        let mut file = File::open(path).await.map_err(unexpected)?;
        let mut content = vec![];
        file.read_to_end(&mut content).await.map_err(unexpected)?;
        Ok(Some(bincode::deserialize(&content).map_err(unexpected)?))
    }

    async fn delete(&self, id: Uuid, hmac: DocumentHmac) -> LbResult<()> {
        let path = self.get_path(id, hmac);
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

    fn exists(&self, id: Uuid, hmac: DocumentHmac) -> bool {
        self.get_path(id, hmac).exists()
    }
}

impl DocumentService for OnDiskDocuments {
    fn get_path(&self, id: Uuid, hmac: DocumentHmac) -> PathBuf {
        let mut path = self.config.files.path.clone();
        // we may need to truncate this
        let hmac = base64::encode_config(hmac, base64::URL_SAFE);
        path.push(format!("{id}-{hmac}"));
        path
    }
}
