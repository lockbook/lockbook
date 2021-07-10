use crate::config::FilesDbConfig;
use lockbook_models::crypto::EncryptedDocument;
use s3::bucket::Bucket as S3Client;
use s3::creds::Credentials;
use s3::region::Region;
use uuid::Uuid;

#[derive(Debug)]
pub enum Error {
    InvalidAccessKeyId(String),
    NoSuchKey(String),
    ResponseNotUtf8(String),
    SignatureDoesNotMatch(String),
    Serialization(serde_json::Error),
    Deserialization(serde_json::Error),
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
                Region::Custom {
                    endpoint: url,
                    region: config.region.clone(),
                },
                credentials,
            )
        }
        _ => S3Client::new(&config.bucket, config.region.parse().unwrap(), credentials),
    }
    .map_err(|err| Error::Unknown(Some(err.to_string())))
}

pub async fn create(
    client: &S3Client,
    file_id: Uuid,
    content_version: u64,
    file_contents: &EncryptedDocument,
) -> Result<(), Error> {
    match client
        .put_object_with_content_type(
            &format!("/{}-{}", file_id, content_version),
            &serde_json::to_vec(file_contents).map_err(Error::Serialization)?,
            "text/plain",
        )
        .await
        .map_err(|err| err.to_string())?
    {
        (_, 200) => Ok(()),
        (body, _) => Err(Error::from(body)),
    }
}

pub async fn create_empty(
    client: &S3Client,
    file_id: Uuid,
    content_version: u64,
) -> Result<(), Error> {
    match client
        .put_object_with_content_type(
            &format!("/{}-{}", file_id, content_version),
            &[],
            "text/plain",
        )
        .await
        .map_err(|err| err.to_string())?
    {
        (_, 200) => Ok(()),
        (body, _) => Err(Error::from(body)),
    }
}

pub async fn delete(client: &S3Client, file_id: Uuid, content_version: u64) -> Result<(), Error> {
    match client
        .delete_object(&format!("/{}-{}", file_id, content_version))
        .await
        .map_err(|err| err.to_string())?
    {
        (_, 204) => Ok(()),
        (body, _) => Err(Error::from(body)),
    }
}

pub async fn get(
    client: &S3Client,
    file_id: Uuid,
    content_version: u64,
) -> Result<Option<EncryptedDocument>, Error> {
    match client
        .get_object(&format!("/{}-{}", file_id, content_version))
        .await
        .map_err(|err| err.to_string())?
    {
        (data, 200) => {
            if data.len() == 0 {
                Ok(None)
            } else {
                Ok(serde_json::from_slice(&data).map_err(Error::Deserialization)?)
            }
        }
        (body, _) => Err(Error::from(body)),
    }
}
