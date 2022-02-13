use crate::config::FilesDbConfig;
use crate::ServerState;

use log::{debug, error};
use s3::bucket::Bucket as S3Client;
use s3::creds::Credentials;
use s3::region::Region;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Debug)]
pub enum Error {
    InvalidAccessKeyId(String),
    NoSuchKey(String),
    ResponseNotUtf8(String),
    SignatureDoesNotMatch(String),
    Unknown(Option<String>),
}

impl From<String> for Error {
    fn from(err: String) -> Error {
        if err.contains("<Code>InvalidAccessKeyId</Code>") {
            Error::InvalidAccessKeyId(err)
        } else if err.contains("<Code>SignatureDoesNotMatch</Code>") {
            Error::SignatureDoesNotMatch(err)
        } else if err.contains("<Code>NoSuchKey</Code>") {
            Error::NoSuchKey(err)
        } else {
            Error::Unknown(Some(err))
        }
    }
}

impl From<Vec<u8>> for Error {
    fn from(err: Vec<u8>) -> Error {
        match String::from_utf8(err) {
            Ok(s) => Error::from(s),
            Err(err) => Error::ResponseNotUtf8(err.to_string()),
        }
    }
}

pub fn create_client(config: &FilesDbConfig) -> Result<S3Client, Error> {
    debug!("Creating files_db client...");

    let credentials = Credentials {
        access_key: Some(config.access_key.clone()),
        secret_key: Some(config.secret_key.clone()),
        security_token: None,
        session_token: None,
    };

    match (&config.scheme, &config.host, &config.port) {
        (Some(scheme), Some(host), Some(port)) => {
            let url = format!("{}://{}:{}", scheme, host, port);
            S3Client::new_with_path_style(
                &config.bucket,
                Region::Custom { endpoint: url, region: config.region.clone() },
                credentials,
            )
        }
        _ => S3Client::new(&config.bucket, config.region.parse().unwrap(), credentials),
    }
    .map_err(|err| Error::Unknown(Some(err.to_string())))
}

pub async fn create(
    state: &ServerState, file_id: Uuid, content_version: u64, file_contents: &[u8],
) -> Result<(), Error> {
    let client = &state.files_db_client;
    match client
        .put_object_with_content_type(
            &format!("/{}-{}", file_id, content_version),
            file_contents,
            "binary/octet-stream",
        )
        .await
        .map_err(|err| err.to_string())?
    {
        (_, 200) => Ok(()),
        (body, _) => Err(Error::from(body)),
    }
}

pub async fn delete(state: &ServerState, file_id: Uuid, content_version: u64) -> Result<(), Error> {
    let client = &state.files_db_client;
    match client
        .delete_object(&format!("/{}-{}", file_id, content_version))
        .await
        .map_err(|err| err.to_string())?
    {
        (_, 204) => Ok(()),
        (body, _) => Err(Error::from(body)),
    }
}

pub fn background_delete(state: &ServerState, file_id: Uuid, content_version: u64) {
    let state = state.clone();
    tokio::spawn(async move {
        match delete(&state, file_id, content_version).await {
            Ok(_) => return,
            Err(err) => error!(
                "Failed to delete file out of s3, will retry after 1 second. Error: {:?}",
                err
            ),
        }
        sleep(Duration::from_secs(1)).await;
        match delete(&state, file_id, content_version).await {
            Ok(_) => return,
            Err(err) => error!("Failed to delete file out of s3 for the second time, will retry after 1 second. Error: {:?}", err)
        }
        sleep(Duration::from_secs(1)).await;
        if let Err(err) = delete(&state, file_id, content_version).await {
            error!("Failed to delete file out of s3 for the third and last time. Error: {:?}, id: {}, version: {}", err, file_id, content_version)
        }
    });
}

pub async fn get(
    state: &ServerState, file_id: Uuid, content_version: u64,
) -> Result<Option<Vec<u8>>, Error> {
    let client = &state.files_db_client;
    match client
        .get_object(&format!("/{}-{}", file_id, content_version))
        .await
        .map_err(|err| err.to_string())?
    {
        (data, 200) => {
            if data.is_empty() {
                Ok(None)
            } else {
                Ok(Some(data))
            }
        }
        (body, _) => Err(Error::from(body)),
    }
}
