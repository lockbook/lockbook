use uuid::Uuid;

use crate::model::state::Config;
use crate::storage::db_provider::FileBackend;
use lockbook_models::crypto::*;

#[derive(Debug)]
pub enum Error {
    BackendError(std::io::Error),
    SerdeError(serde_json::Error),
    FileRowMissing(()), // TODO remove from insert
}

#[derive(Debug)]
pub enum DbError {
    BackendError(std::io::Error),
    SerdeError(serde_json::Error),
}

pub trait DocumentRepo {
    const NAMESPACE: &'static [u8] = b"documents";
    fn insert(config: &Config, id: Uuid, document: &EncryptedDocument) -> Result<(), Error>;
    fn get(config: &Config, id: Uuid) -> Result<EncryptedDocument, Error>;
    fn maybe_get(config: &Config, id: Uuid) -> Result<Option<EncryptedDocument>, DbError>;
    fn delete(config: &Config, id: Uuid) -> Result<(), Error>;
}

pub struct DocumentRepoImpl;

impl DocumentRepo for DocumentRepoImpl {
    fn insert(config: &Config, id: Uuid, document: &EncryptedDocument) -> Result<(), Error> {
        FileBackend::write(
            config,
            Self::NAMESPACE,
            id.to_string().as_str(),
            serde_json::to_vec(document).map_err(Error::SerdeError)?,
        )
        .map_err(Error::BackendError)
    }

    fn get(config: &Config, id: Uuid) -> Result<EncryptedDocument, Error> {
        let maybe_data: Option<Vec<u8>> =
            FileBackend::read(config, Self::NAMESPACE, id.to_string().as_str())
                .map_err(Error::BackendError)?;
        match maybe_data {
            None => Err(Error::FileRowMissing(())),
            Some(data) => serde_json::from_slice(&data).map_err(Error::SerdeError),
        }
    }

    fn maybe_get(config: &Config, id: Uuid) -> Result<Option<EncryptedDocument>, DbError> {
        let maybe_data: Option<Vec<u8>> =
            FileBackend::read(config, Self::NAMESPACE, id.to_string().as_str())
                .map_err(DbError::BackendError)?;
        match maybe_data {
            None => Ok(None),
            Some(data) => serde_json::from_slice(&data)
                .map(Some)
                .map_err(DbError::SerdeError),
        }
    }

    fn delete(config: &Config, id: Uuid) -> Result<(), Error> {
        FileBackend::delete(config, Self::NAMESPACE, id.to_string().as_str())
            .map_err(Error::BackendError)
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::state::temp_config;
    use crate::repo::document_repo::DocumentRepo;
    use crate::{DefaultBackend, DefaultDocumentRepo};
    use lockbook_models::crypto::*;

    #[test]
    fn update_document() {
        let test_document = EncryptedDocument::new("something", "nonce1");

        let config = temp_config();
        let db = DefaultBackend::connect_to_db(&config).unwrap();

        let document_id = Uuid::new_v4();

        DefaultDocumentRepo::insert(&db, document_id, &test_document).unwrap();

        let document = DefaultDocumentRepo::get(&db, document_id).unwrap();
        assert_eq!(document, EncryptedDocument::new("something", "nonce1"),);

        DefaultDocumentRepo::insert(
            &db,
            document_id,
            &EncryptedDocument::new("updated", "nonce2"),
        )
        .unwrap();

        let file_updated = DefaultDocumentRepo::get(&db, document_id).unwrap();

        assert_eq!(file_updated, EncryptedDocument::new("updated", "nonce2"));
    }
}
