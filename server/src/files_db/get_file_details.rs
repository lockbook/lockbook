use s3::bucket::Bucket as S3Client;
use s3::error::S3Error;

#[derive(Debug)]
pub enum Error {
    S3ConnectionFailed(S3Error),
    S3OperationUnsuccessful(u16),
    NoSuchFile(()),
}

#[derive(Debug)]
pub struct FileDetails {
    pub id: String,
    pub size: u64,
}

impl From<&s3::serde_types::Object> for FileDetails {
    fn from(object: &s3::serde_types::Object) -> FileDetails {
        FileDetails {
            id: object.key.clone(),
            size: object.size,
        }
    }
}

pub fn get_file_details(client: &S3Client, file_id: &str) -> Result<FileDetails, Error> {
    Ok(())
        .and_then(|_| match client.list_all(file_id.to_string(), None) {
            Ok(file_details) => Ok(file_details),
            Err(err) => Err(Error::S3ConnectionFailed(err)),
        })
        .and_then(|file_details| match file_details.first() {
            Some((list, 200)) => list
                .contents
                .first()
                .ok_or(Error::NoSuchFile(()))
                .map(FileDetails::from),
            Some((_, code)) => Err(Error::S3OperationUnsuccessful(*code)),
            None => Err(Error::NoSuchFile(())),
        })
}
