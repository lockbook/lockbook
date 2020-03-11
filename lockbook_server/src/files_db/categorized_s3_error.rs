use s3::error::S3Error;

#[derive(Debug)]
pub enum CategorizedS3Error {
    ErrorTryingToConnect(String),
    InvalidAccessKeyId(String),
    NoSuchKey(String),
    ResponseNotUtf8(String),
    SignatureDoesNotMatch(String),
    Unknown(Option<String>),
}

impl From<String> for CategorizedS3Error {
    fn from(err: String) -> CategorizedS3Error {
        if err.contains("<Code>InvalidAccessKeyId</Code>") {
            CategorizedS3Error::InvalidAccessKeyId(err)
        } else if err.contains("<Code>SignatureDoesNotMatch</Code>") {
            CategorizedS3Error::SignatureDoesNotMatch(err)
        } else if err.contains("<Code>NoSuchKey</Code>") {
            CategorizedS3Error::NoSuchKey(err)
        } else {
            CategorizedS3Error::Unknown(Some(err))
        }
    }
}

impl From<Vec<u8>> for CategorizedS3Error {
    fn from(err: Vec<u8>) -> CategorizedS3Error {
        match String::from_utf8(err) {
            Ok(s) => CategorizedS3Error::from(s),
            Err(err) => CategorizedS3Error::ResponseNotUtf8(err.to_string()),
        }
    }
}

impl From<S3Error> for CategorizedS3Error {
    fn from(err: S3Error) -> CategorizedS3Error {
        match err.description {
            Some(desc) => {
                if desc.contains("error trying to connect: ") {
                    CategorizedS3Error::ErrorTryingToConnect(desc)
                } else {
                    CategorizedS3Error::Unknown(Some(desc))
                }
            }
            None => CategorizedS3Error::Unknown(None),
        }
    }
}
