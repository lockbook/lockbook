use crate::config::FilesDbConfig;
use s3::bucket::Bucket as S3Client;
use s3::credentials::Credentials;
use s3::error::S3Error;

#[derive(Debug)]
pub enum Error {
    UnknownRegion(String),
    // AuthenticationFailed(String),
    S3ConnectionFailed(S3Error),
}

pub fn connect(config: &FilesDbConfig) -> Result<S3Client, Error> {
    Ok(())
        .and_then(|_| match config.region.parse::<s3::region::Region>() {
            Ok(region) => Ok((
                region,
                Credentials::new(
                    Some(config.access_key.to_string()),
                    Some(config.secret_key.to_string()),
                    None,
                    None,
                ),
            )),
            Err(err) => Err(Error::UnknownRegion(err.to_string())),
        })
        .and_then(
            |(region, credentials)| match S3Client::new(config.bucket, region, credentials) {
                Ok(client) => Ok(client),
                Err(err) => Err(Error::S3ConnectionFailed(err)),
            },
        )
}
