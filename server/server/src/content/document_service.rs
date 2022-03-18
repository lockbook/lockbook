use crate::content::{content_cache, file_content_client};
use crate::file_content_client::Error::{
    InvalidAccessKeyId, NoSuchKey, ResponseNotUtf8, SignatureDoesNotMatch, Unknown,
};
use crate::ServerError::InternalError;
use crate::{ServerError, ServerState};
use futures::future::join;
use lockbook_models::api::GetDocumentError;
use lockbook_models::api::GetDocumentError::DocumentNotFound;
use lockbook_models::crypto::EncryptedDocument;
use log::error;
use std::fmt::Debug;
use uuid::Uuid;

pub enum Error {
    S3Error(file_content_client::Error),
    RedisError,
}

pub async fn create<T: Debug>(
    state: &ServerState, id: Uuid, content_version: u64, file_contents: &EncryptedDocument,
) -> Result<(), ServerError<T>> {
    let content = bincode::serialize(file_contents)
        .map_err(|err| internal!("Failed to serialize a document: {}", err))?;
    let redis_save = content_cache::create::<T>(state, id, content_version, &content);
    let s3_save = file_content_client::create(state, id, content_version, &content);

    let result = join(redis_save, s3_save).await;
    match result {
        (Ok(_), Ok(_)) => Ok(()),
        (Ok(_), Err(s3)) => {
            let message = format!("Failed to insert doc into s3, succeeded in redis, failing the request, cleaning up redis. Err: {:?}", s3);
            error!("{}", message);
            content_cache::delete(state, id, content_version).await?;
            Err(InternalError(message))
        }
        (Err(redis), Ok(_)) => {
            error!("Failed to insert document into redis: {:?}. Successfully inserted in s3, continuing with request", redis);
            Ok(())
        }
        (Err(err), Err(err2)) => Err(internal!(
            "Fails to both doc locations failed. redis err: {:?}, s3 err: {:?}, id: {}, ver: {}",
            err,
            err2,
            id,
            content_version
        )),
    }
}

pub async fn get(
    state: &ServerState, id: Uuid, content_version: u64,
) -> Result<EncryptedDocument, ServerError<GetDocumentError>> {
    let document_bytes = match content_cache::get(state, id, content_version).await? {
        Some(document_bytes) => document_bytes,
        None => {
            let bytes = file_content_client::get(state, id, content_version)
                .await
                .map_err(|err| match err {
                    NoSuchKey(_, _) => ServerError::ClientError(DocumentNotFound),
                    InvalidAccessKeyId(_, _)
                    | ResponseNotUtf8(_, _)
                    | SignatureDoesNotMatch(_, _)
                    | Unknown(_, _) => internal!("Cannot get file from s3: {:?}", err),
                })?;
            content_cache::create(state, id, content_version, &bytes).await?;
            bytes
        }
    };

    Ok(bincode::deserialize(&document_bytes)?)
}

pub async fn delete<T: Debug>(
    state: &ServerState, id: Uuid, content_version: u64,
) -> Result<(), ServerError<T>> {
    content_cache::delete(state, id, content_version).await?;

    file_content_client::delete(state, id, content_version)
        .await
        .map_err(|err| internal!("could not delete file from s3: {:?}", err))?;

    Ok(())
}

pub async fn background_delete<T: Debug>(
    state: &ServerState, id: Uuid, content_version: u64,
) -> Result<(), ServerError<T>> {
    content_cache::delete(state, id, content_version).await?;

    file_content_client::background_delete(state, id, content_version);

    Ok(())
}
