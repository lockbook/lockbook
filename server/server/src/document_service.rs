use crate::{ServerError, ServerState};

use lockbook_shared::crypto::EncryptedDocument;
use lockbook_shared::file_metadata::DocumentHmac;
use std::fmt::Debug;
use std::path::PathBuf;
use tokio::fs::{remove_file, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

impl ServerState {
    pub(crate) async fn insert<T: Debug>(
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

    pub(crate) async fn get<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<EncryptedDocument, ServerError<T>> {
        let path = self.get_path(id, hmac);
        let mut file = File::open(path.clone()).await?;
        let mut content = vec![];
        file.read_to_end(&mut content).await?;
        let content = bincode::deserialize(&content)?;
        Ok(content)
    }

    pub(crate) fn exists(&self, id: &Uuid, hmac: &DocumentHmac) -> bool {
        self.get_path(id, hmac).exists()
    }

    pub(crate) async fn delete<T: Debug>(
        &self, id: &Uuid, hmac: &DocumentHmac,
    ) -> Result<(), ServerError<T>> {
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

    fn get_path(&self, id: &Uuid, hmac: &DocumentHmac) -> PathBuf {
        let mut path = self.config.files.path.clone();
        // we may need to truncate this
        let hmac = base64::encode_config(hmac, base64::URL_SAFE);
        path.push(format!("{}-{}", id, hmac));
        path
    }
}
