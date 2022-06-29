use crate::{ServerError, ServerState};
use lockbook_models::crypto::EncryptedDocument;
use std::fmt::Debug;
use std::path::PathBuf;
use tokio::fs::{remove_file, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

pub(crate) async fn insert<T: Debug>(
    state: &ServerState, id: Uuid, content_version: u64, content: &EncryptedDocument,
) -> Result<(), ServerError<T>> {
    let content = bincode::serialize(content)?;
    let mut file = File::create(get_path(state, id, content_version)).await?;
    file.write_all(&content).await.unwrap();
    Ok(())
}

pub(crate) async fn get<T: Debug>(
    state: &ServerState, id: Uuid, content_version: u64,
) -> Result<EncryptedDocument, ServerError<T>> {
    let mut file = File::open(get_path(state, id, content_version)).await?;
    let mut content = vec![];
    file.read_to_end(&mut content).await?;
    let content = bincode::deserialize(&content)?;
    Ok(content)
}

pub(crate) async fn delete<T: Debug>(
    state: &ServerState, id: Uuid, content_version: u64,
) -> Result<(), ServerError<T>> {
    let path = get_path(state, id, content_version);
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

fn get_path(state: &ServerState, id: Uuid, content_version: u64) -> PathBuf {
    let mut path = state.config.files.path.clone();
    path.push(format!("{}-{}", id, content_version));
    path
}
