use s3::bucket::Bucket as S3Client;
use crate::files_db::categorized_s3_error::CategorizedS3Error;

#[derive(Debug)]
pub enum Error {
    S3(CategorizedS3Error),
}

pub fn get_file(client: &S3Client, file_id: &str) -> Result<String, Error> {
    Ok(())
        .and_then(|_| match client.get_object(&format!("/{}", file_id)) {
            Ok((body, 200)) => Ok(body),
            Ok((body, _)) => Err(Error::S3(CategorizedS3Error::from(body))),
            Err(err) => Err(Error::S3(CategorizedS3Error::from(err))),
        })
        .and_then(|body| match String::from_utf8(body) {
            Ok(body) => Ok(body),
            Err(err) => Err(Error::S3(CategorizedS3Error::ResponseNotUtf8(err.to_string()))),
        })
}