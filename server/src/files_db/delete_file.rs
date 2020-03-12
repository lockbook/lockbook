use s3::bucket::Bucket as S3Client;

#[derive(Debug)]
pub enum Error {
    // S3ConnectionFailed(String),
    // FileAlreadyExists(String),
    S3OperationUnsuccessful((u16, String)),
}

pub fn delete_file(client: &S3Client, file_id: &str) -> Result<(), Error> {
    match client.delete_object(&format!("/{}", file_id)).unwrap() {
        (_, 204) => Ok(()),
        (body, status_code) => Err(Error::S3OperationUnsuccessful((
            status_code,
            String::from_utf8(body).unwrap(),
        ))),
    }
}
