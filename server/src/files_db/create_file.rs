use crate::files_db::categorized_s3_error;
use s3::bucket::Bucket as S3Client;

pub async fn create_file(
    client: &S3Client,
    file_id: &str,
    file_contents: &str,
    content_version: u64,
) -> Result<(), categorized_s3_error::Error> {
    match client
        .put_object(
            &format!("/{}-{}", file_id, content_version),
            file_contents.as_bytes(),
            "text/plain",
        )
        .await?
    {
        (_, 200) => Ok(()),
        (body, _) => Err(categorized_s3_error::Error::from(body)),
    }
}
