use crate::config::FilesDbConfig;
use crate::ServerState;

use crate::file_content_client::Error::Unknown;
use log::{debug, error};
use s3::bucket::Bucket as S3Client;
use s3::creds::Credentials;
use s3::region::Region;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

// body + error code
#[derive(Debug)]
pub enum Error {
    InvalidAccessKeyId(String, u16),
    NoSuchKey(String, u16),
    ResponseNotUtf8(String, u16),
    SignatureDoesNotMatch(String, u16),
    Unknown(Option<String>, Option<u16>),
}

impl From<(String, u16)> for Error {
    fn from(bundle: (String, u16)) -> Error {
        let (err, status) = bundle;
        if err.contains("<Code>InvalidAccessKeyId</Code>") {
            Error::InvalidAccessKeyId(err, status)
        } else if err.contains("<Code>SignatureDoesNotMatch</Code>") {
            Error::SignatureDoesNotMatch(err, status)
        } else if err.contains("<Code>NoSuchKey</Code>") {
            Error::NoSuchKey(err, status)
        } else {
            Error::Unknown(Some(err), Some(status))
        }
    }
}

impl From<(Vec<u8>, u16)> for Error {
    fn from(bundle: (Vec<u8>, u16)) -> Error {
        let (err, status) = bundle;
        match String::from_utf8(err) {
            Ok(s) => Error::from((s, status)),
            Err(err) => Error::ResponseNotUtf8(err.to_string(), status),
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
    .map_err(|err| Error::Unknown(Some(err.to_string()), None))
}

pub async fn create(
    state: &ServerState, file_id: Uuid, content_version: u64, file_contents: &[u8],
) -> Result<(), Error> {
    let client = &state.files_db_client;
    let mut response = Ok(());
    for attempt_number in 1..=3 {
        let (body, status) = client
            .put_object_with_content_type(
                &format!("/{}-{}", file_id, content_version),
                file_contents,
                "binary/octet-stream",
            )
            .await
            .map_err(|err| Unknown(Some(err.to_string()), None))?;

        match (body, status) {
            (_, 200) => return Ok(()),
            (body, 500..=599) => {
                // https://docs.aws.amazon.com/AmazonS3/latest/userguide/ErrorBestPractices.html
                error!(
                    "{} while creating in s3, contents: {}. Will retry: {}.",
                    status,
                    String::from_utf8(body.clone()).unwrap_or_else(|_| "invalid utf8".to_string()),
                    attempt_number != 3
                );
                response = Err(Error::from((body, status)));
            }
            (body, _) => return Err(Error::from((body, status))),
        }

        sleep(Duration::from_secs(1)).await;
    }
    response
}

pub async fn delete(state: &ServerState, file_id: Uuid, content_version: u64) -> Result<(), Error> {
    let client = &state.files_db_client;
    let mut response = Ok(());
    for attempt_number in 1..=3 {
        let (body, status) = client
            .delete_object(&format!("/{}-{}", file_id, content_version))
            .await
            .map_err(|err| Unknown(Some(err.to_string()), None))?;

        match (body, status) {
            (_, 204) => return Ok(()),
            (body, 500..=599) => {
                error!(
                    "{} while deleting in s3, contents: {}. Will retry: {}.",
                    status,
                    String::from_utf8(body.clone()).unwrap_or_else(|_| "invalid utf8".to_string()),
                    attempt_number != 3
                );
                response = Err(Error::from((body, status)));
            }
            (body, _) => return Err(Error::from((body, status))),
        }

        sleep(Duration::from_secs(1)).await;
    }
    response
}

pub fn background_delete(state: &ServerState, file_id: Uuid, content_version: u64) {
    let state = state.clone();
    tokio::spawn(async move {
        delete(&state, file_id, content_version)
            .await
            .unwrap_or_else(|err| error!("Failed to delete file out of s3. Error: {:?}", err));
    });
}

pub async fn get(
    state: &ServerState, file_id: Uuid, content_version: u64,
) -> Result<Option<Vec<u8>>, Error> {
    let client = &state.files_db_client;
    let mut response = Ok(None);
    for attempt_number in 1..=3 {
        let (body, status) = client
            .get_object(&format!("/{}-{}", file_id, content_version))
            .await
            .map_err(|err| Unknown(Some(err.to_string()), None))?;

        match (body, status) {
            (data, 200) => {
                if data.is_empty() {
                    return Ok(None);
                } else {
                    return Ok(Some(data));
                }
            }
            (body, 500..=599) => {
                error!(
                    "{} while deleting in s3, contents: {}. Will retry: {}.",
                    status,
                    String::from_utf8(body.clone()).unwrap_or_else(|_| "invalid utf8".to_string()),
                    attempt_number != 3
                );
                response = Err(Error::from((body, status)));
            }
            (body, _) => return Err(Error::from((body, status))),
        }
    }

    response
}
