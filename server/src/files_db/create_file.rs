use crate::files_db::categorized_s3_error;
use s3::bucket::Bucket as S3Client;

#[derive(Debug)]
pub enum Error {
    S3(categorized_s3_error::Error),
}

pub fn create_file(client: &S3Client, file_id: &str, file_contents: &str) -> Result<(), Error> {
    match client.put_object(
        &format!("/{}", file_id),
        file_contents.as_bytes(),
        "text/plain",
    ) {
        Ok((_, 200)) => Ok(()),
        Ok((body, _)) => Err(Error::S3(categorized_s3_error::Error::from(body))),
        Err(err) => Err(Error::S3(categorized_s3_error::Error::from(err))),
    }
}
