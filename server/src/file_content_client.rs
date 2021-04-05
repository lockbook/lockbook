use crate::config::FilesDbConfig;
use lockbook_models::crypto::EncryptedDocument;
use s3::bucket::Bucket as S3Client;
use s3::creds::AwsCredsError;
use s3::creds::Credentials;
use s3::region::Region;
use s3::S3Error;
use uuid::Uuid;

#[derive(Debug)]
pub enum Error {
    Credentials(AwsCredsError),
    ErrorTryingToConnect(String),
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

impl From<S3Error> for Error {
    fn from(err: S3Error) -> Error {
        match err.description {
            Some(desc) => {
                if desc.contains("error trying to connect: ") {
                    Error::ErrorTryingToConnect(desc)
                } else {
                    Error::Unknown(Some(desc))
                }
            }
            None => Error::Unknown(None),
        }
    }
}

pub async fn connect(config: &FilesDbConfig) -> Result<S3Client, Error> {
    let credentials = Credentials::new(
        Some(&config.access_key),
        Some(&config.secret_key),
        None,
        None,
        None,
    )
    .await
    .map_err(Error::Credentials)?;
    Ok(S3Client::new(
        &config.bucket,
        Region::Custom {
            endpoint: format!("{}://{}:{}", config.scheme, config.host, config.port),
            region: config.region.clone(),
        },
        credentials,
    )?)
}

pub async fn create(
    client: &S3Client,
    file_id: Uuid,
    content_version: u64,
    file_contents: &EncryptedDocument,
) -> Result<(), Error> {
    match client
        .put_object(
            &format!("/{}-{}", file_id, content_version),
            &serde_json::to_vec(file_contents).map_err(Error::Serialization)?,
            "text/plain",
        )
        .await?
    {
        (_, 200) => Ok(()),
        (body, _) => Err(Error::from(body)),
    }
}

pub async fn delete(client: &S3Client, file_id: Uuid, content_version: u64) -> Result<(), Error> {
    match client
        .delete_object(&format!("/{}-{}", file_id, content_version))
        .await?
    {
        (_, 204) => Ok(()),
        (body, _) => Err(Error::from(body)),
    }
}

pub async fn get(
    client: &S3Client,
    file_id: Uuid,
    content_version: u64,
) -> Result<EncryptedDocument, Error> {
    match client
        .get_object(&format!("/{}-{}", file_id, content_version))
        .await?
    {
        (data, 200) => Ok(serde_json::from_slice(&data).map_err(Error::Deserialization)?),
        (body, _) => Err(Error::from(body)),
    }
}
