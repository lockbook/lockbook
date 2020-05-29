use s3::creds::AwsCredsError;
use s3::S3Error;

#[derive(Debug)]
pub enum Error {
    Credentials(AwsCredsError),
    ErrorTryingToConnect(String),
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

impl From<AwsCredsError> for Error {
    fn from(err: AwsCredsError) -> Error {
        Error::Credentials(err)
    }
}
