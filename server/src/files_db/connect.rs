use crate::config::FilesDbConfig;
use crate::files_db::categorized_s3_error;
use s3::bucket::Bucket as S3Client;
use s3::creds::Credentials;
use s3::region::Region;

pub async fn connect(config: &FilesDbConfig) -> Result<S3Client, categorized_s3_error::Error> {
    let credentials = Credentials::new(
        Some(&config.access_key),
        Some(&config.secret_key),
        None,
        None,
        None,
    )
    .await?;
    let client = S3Client::new(&config.bucket, Region::Custom {
        endpoint: format!("{}:{}", config.host, config.port),
        region: config.region.clone(),
    }, credentials)?;
    Ok(client)
}
