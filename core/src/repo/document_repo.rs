use uuid::Uuid;

use crate::model::crypto::*;
use crate::storage::db_provider::Backend;

#[derive(Debug)]
pub enum Error<MyBackend: Backend> {
    BackendError(MyBackend::Error),
    SerdeError(serde_json::Error),
    FileRowMissing(()), // TODO remove from insert
}

#[derive(Debug)]
pub enum DbError<MyBackend: Backend> {
    BackendError(MyBackend::Error),
    SerdeError(serde_json::Error),
}

pub trait DocumentRepo<MyBackend: Backend> {
    const NAMESPACE: &'static [u8] = b"documents";
    fn insert(
        backend: &MyBackend::Db,
        id: Uuid,
        document: &EncryptedDocument,
    ) -> Result<(), Error<MyBackend>>;
    fn get(backend: &MyBackend::Db, id: Uuid) -> Result<EncryptedDocument, Error<MyBackend>>;
    fn maybe_get(
        backend: &MyBackend::Db,
        id: Uuid,
    ) -> Result<Option<EncryptedDocument>, DbError<MyBackend>>;
    fn delete(backend: &MyBackend::Db, id: Uuid) -> Result<(), Error<MyBackend>>;
}

pub struct DocumentRepoImpl<MyBackend: Backend> {
    _backend: MyBackend,
}

impl<MyBackend: Backend> DocumentRepo<MyBackend> for DocumentRepoImpl<MyBackend> {
    fn insert(
        backend: &MyBackend::Db,
        id: Uuid,
        document: &EncryptedDocument,
    ) -> Result<(), Error<MyBackend>> {
        MyBackend::write(
            backend,
            Self::NAMESPACE,
            id.to_string().as_str(),
            serde_json::to_vec(document).map_err(Error::SerdeError)?,
        )
        .map_err(Error::BackendError)
    }

    fn get(backend: &MyBackend::Db, id: Uuid) -> Result<EncryptedDocument, Error<MyBackend>> {
        let maybe_data: Option<Vec<u8>> =
            MyBackend::read(backend, Self::NAMESPACE, id.to_string().as_str())
                .map_err(Error::BackendError)?;
        match maybe_data {
            None => Err(Error::FileRowMissing(())),
            Some(data) => serde_json::from_slice(&data).map_err(Error::SerdeError),
        }
    }

    fn maybe_get(
        backend: &MyBackend::Db,
        id: Uuid,
    ) -> Result<Option<EncryptedDocument>, DbError<MyBackend>> {
        let maybe_data: Option<Vec<u8>> =
            MyBackend::read(backend, Self::NAMESPACE, id.to_string().as_str())
                .map_err(DbError::BackendError)?;
        match maybe_data {
            None => Ok(None),
            Some(data) => serde_json::from_slice(&data)
                .map(Some)
                .map_err(DbError::SerdeError),
        }
    }

    fn delete(backend: &MyBackend::Db, id: Uuid) -> Result<(), Error<MyBackend>> {
        MyBackend::delete(backend, Self::NAMESPACE, id.to_string().as_str())
            .map_err(Error::BackendError)
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::state::temp_config;
    use crate::repo::document_repo::DocumentRepo;
    use crate::storage::db_provider::Backend;
    use crate::{model::crypto::*, DefaultBackend, DefaultDocumentRepo};

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
