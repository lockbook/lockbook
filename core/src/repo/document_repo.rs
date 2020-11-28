use sled::{Db, IVec};
use uuid::Uuid;

use crate::model::crypto::*;
use crate::repo::db_provider::{Backend, BackendError};
use std::borrow::Borrow;

#[derive(Debug)]
pub enum Error {
    BackendError(BackendError),
    SerdeError(serde_json::Error),
    FileRowMissing(()), // TODO remove from insert
}

#[derive(Debug)]
pub enum DbError {
    BackendError(BackendError),
    SerdeError(serde_json::Error),
}

pub trait DocumentRepo {
    fn insert(backend: Backend, id: Uuid, document: &EncryptedDocument) -> Result<(), Error>;
    fn get(backend: Backend, id: Uuid) -> Result<EncryptedDocument, Error>;
    fn maybe_get(backend: Backend, id: Uuid) -> Result<Option<EncryptedDocument>, DbError>;
    fn delete(backend: Backend, id: Uuid) -> Result<(), Error>;
}

pub struct DocumentRepoImpl;

impl DocumentRepo for DocumentRepoImpl {
    fn insert(backend: Backend, id: Uuid, document: &EncryptedDocument) -> Result<(), Error> {
        backend
            .write(
                id.as_bytes(),
                serde_json::to_vec(document).map_err(Error::SerdeError)?,
            )
            .map_err(Error::BackendError)
            .map(|_| ())
    }

    fn get(backend: Backend, id: Uuid) -> Result<EncryptedDocument, Error> {
        let maybe_data: Option<IVec> = backend.read(id.as_bytes()).map_err(Error::BackendError)?;
        match maybe_data {
            None => Err(Error::FileRowMissing(())),
            Some(data) => serde_json::from_slice(data.borrow()).map_err(Error::SerdeError),
        }
    }

    fn maybe_get(backend: Backend, id: Uuid) -> Result<Option<EncryptedDocument>, DbError> {
        let maybe_data: Option<IVec> =
            backend.read(id.as_bytes()).map_err(DbError::BackendError)?;
        match maybe_data {
            None => Ok(None),
            Some(data) => serde_json::from_slice(data.borrow()).map_err(DbError::SerdeError),
        }
    }

    fn delete(backend: Backend, id: Uuid) -> Result<(), Error> {
        backend.delete(id.as_bytes()).map_err(Error::BackendError)
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::crypto::*;
    use crate::model::state::dummy_config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::document_repo::{DocumentRepo, DocumentRepoImpl};

    type DefaultDbProvider = TempBackedDB;

    #[test]
    fn update_document() {
        let test_document = EncryptedDocument::new("something", "nonce1");

        let config = dummy_config();
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        let document_id = Uuid::new_v4();

        DocumentRepoImpl::insert(&db, document_id, &test_document).unwrap();

        let document = DocumentRepoImpl::get(&db, document_id).unwrap();
        assert_eq!(document, EncryptedDocument::new("something", "nonce1"),);

        DocumentRepoImpl::insert(
            &db,
            document_id,
            &EncryptedDocument::new("updated", "nonce2"),
        )
        .unwrap();

        let file_updated = DocumentRepoImpl::get(&db, document_id).unwrap();

        assert_eq!(file_updated, EncryptedDocument::new("updated", "nonce2"));
    }
}
