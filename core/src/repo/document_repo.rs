use crate::error_enum;
use crate::model::crypto::*;
use sled::Db;
use uuid::Uuid;

error_enum! {
    enum Error {
        SledError(sled::Error),
        SerdeError(serde_json::Error),
        FileRowMissing(()) // TODO remove from insert
    }
}

pub trait DocumentRepo {
    fn insert(db: &Db, id: Uuid, document: &Document) -> Result<(), Error>;
    fn get(db: &Db, id: Uuid) -> Result<Document, Error>;
    fn delete(db: &Db, id: Uuid) -> Result<(), Error>;
}

pub struct DocumentRepoImpl;

impl DocumentRepo for DocumentRepoImpl {
    fn insert(db: &Db, id: Uuid, document: &Document) -> Result<(), Error> {
        let tree = db.open_tree(b"documents")?;
        tree.insert(id.as_bytes(), serde_json::to_vec(document)?)?;
        Ok(())
    }

    fn get(db: &Db, id: Uuid) -> Result<Document, Error> {
        let tree = db.open_tree(b"documents")?;
        let maybe_value = tree.get(id.as_bytes())?;
        let value = maybe_value.ok_or(())?;
        let document: Document = serde_json::from_slice(value.as_ref())?;

        Ok(document)
    }

    fn delete(db: &Db, id: Uuid) -> Result<(), Error> {
        let tree = db.open_tree(b"documents")?;
        tree.remove(id.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::crypto::*;
    use crate::model::state::Config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::document_repo::{DocumentRepo, DocumentRepoImpl};
    use uuid::Uuid;

    type DefaultDbProvider = TempBackedDB;

    #[test]
    fn update_document() {
        let test_document = Document {
            content: EncryptedValueWithNonce {
                garbage: "something".to_string(),
                nonce: "nonce1".to_string(),
            },
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        let document_id = Uuid::new_v4();

        DocumentRepoImpl::insert(&db, document_id, &test_document).unwrap();

        let document = DocumentRepoImpl::get(&db, document_id).unwrap();
        assert_eq!(
            document.content,
            EncryptedValueWithNonce {
                garbage: "something".to_string(),
                nonce: "nonce1".to_string()
            }
        );

        DocumentRepoImpl::insert(
            &db,
            document_id,
            &Document {
                content: EncryptedValueWithNonce {
                    garbage: "updated".to_string(),
                    nonce: "nonce2".to_string(),
                },
            },
        )
        .unwrap();

        let file_updated = DocumentRepoImpl::get(&db, document_id).unwrap();

        assert_eq!(
            file_updated.content,
            EncryptedValueWithNonce {
                garbage: "updated".to_string(),
                nonce: "nonce2".to_string(),
            }
        );
    }
}
