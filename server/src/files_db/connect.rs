use crate::config::FilesDbConfig;
use crate::files_db::categorized_s3_error;
use s3::bucket::Bucket as S3Client;
use s3::credentials::Credentials;

pub fn connect(config: &FilesDbConfig) -> Result<S3Client, categorized_s3_error::Error> {
    let region = config.region.clone();
    let credentials = Credentials::new(
        Some(config.access_key.clone()),
        Some(config.secret_key.clone()),
        None,
        None,
    );
    let client = S3Client::new(&config.bucket, region, credentials)?;
    Ok(client)
}
