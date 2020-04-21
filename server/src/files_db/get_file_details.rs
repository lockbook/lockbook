use crate::files_db::categorized_s3_error;
use s3::bucket::Bucket as S3Client;

#[derive(Debug)]
pub enum Error {
    S3(categorized_s3_error::Error),
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
    let file_details = match client.list_all(file_id.to_string(), None) {
        Ok(fd) => fd,
        Err(err) => {
            return Err(Error::S3(categorized_s3_error::Error::from(err)));
        }
    };

    match file_details.first() {
        Some((list, _)) => list
            .contents
            .first()
            .ok_or(Error::NoSuchFile(()))
            .map(FileDetails::from),
        None => Err(Error::NoSuchFile(())),
    }
}
