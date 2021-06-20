use crate::core_err_unexpected;
use crate::model::state::Config;
use crate::repo::local_storage;
use crate::CoreError;
use lockbook_models::crypto::*;
use uuid::Uuid;

pub const NAMESPACE: &[u8; 9] = b"documents";

pub fn insert(config: &Config, id: Uuid, document: &EncryptedDocument) -> Result<(), CoreError> {
    local_storage::write(
        config,
        NAMESPACE,
        id.to_string().as_str(),
        serde_json::to_vec(document).map_err(core_err_unexpected)?,
    )
}

pub fn get(config: &Config, id: Uuid) -> Result<EncryptedDocument, CoreError> {
    let maybe_data: Option<Vec<u8>> =
        local_storage::read(config, NAMESPACE, id.to_string().as_str())?;
    match maybe_data {
        None => Err(CoreError::FileNonexistent),
        Some(data) => serde_json::from_slice(&data).map_err(core_err_unexpected),
    }
}

pub fn maybe_get(config: &Config, id: Uuid) -> Result<Option<EncryptedDocument>, CoreError> {
    let maybe_data: Option<Vec<u8>> =
        local_storage::read(config, NAMESPACE, id.to_string().as_str())?;
    match maybe_data {
        None => Ok(None),
        Some(data) => serde_json::from_slice(&data)
            .map(Some)
            .map_err(core_err_unexpected),
    }
}

pub fn delete(config: &Config, id: Uuid) -> Result<(), CoreError> {
    local_storage::delete(config, NAMESPACE, id.to_string().as_str())
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::state::temp_config;
    use crate::repo::document_repo;
    use lockbook_models::crypto::*;

    #[test]
    fn update_document() {
        let test_document = EncryptedDocument::new("something", "nonce1");

        let config = temp_config();

        let document_id = Uuid::new_v4();

        document_repo::insert(&config, document_id, &test_document).unwrap();

        let document = document_repo::get(&config, document_id).unwrap();
        assert_eq!(document, EncryptedDocument::new("something", "nonce1"),);

        document_repo::insert(
            &config,
            document_id,
            &EncryptedDocument::new("updated", "nonce2"),
        )
        .unwrap();

        let file_updated = document_repo::get(&config, document_id).unwrap();

        assert_eq!(file_updated, EncryptedDocument::new("updated", "nonce2"));
    }
}
