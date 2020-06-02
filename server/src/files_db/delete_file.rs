use crate::files_db::categorized_s3_error;
use s3::bucket::Bucket as S3Client;

pub async fn delete_file(
    client: &S3Client,
    file_id: &str,
    content_version: u64,
) -> Result<(), categorized_s3_error::Error> {
    match client
        .delete_object(&format!("/{}:{}", file_id, content_version))
        .await?
    {
        (_, 204) => Ok(()),
        (body, _) => Err(categorized_s3_error::Error::from(body)),
    }
}
