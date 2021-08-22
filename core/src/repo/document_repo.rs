use crate::core_err_unexpected;
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::local_storage;
use crate::CoreError;
use lockbook_models::crypto::*;
use uuid::Uuid;

const NAMESPACE_LOCAL: &str = "changed_local_documents";
const NAMESPACE_BASE: &str = "all_base_documents";

fn namespace(source: RepoSource) -> &'static str {
    match source {
        RepoSource::Local => NAMESPACE_LOCAL,
        RepoSource::Base => NAMESPACE_BASE,
    }
}

pub fn insert(
    config: &Config,
    source: RepoSource,
    id: Uuid,
    document: &EncryptedDocument,
) -> Result<(), CoreError> {
    local_storage::write(
        config,
        namespace(source),
        id.to_string().as_str(),
        serde_json::to_vec(document).map_err(core_err_unexpected)?,
    )
}

pub fn get(config: &Config, source: RepoSource, id: Uuid) -> Result<EncryptedDocument, CoreError> {
    let maybe_data: Option<Vec<u8>> =
        local_storage::read(config, namespace(source), id.to_string().as_str())?;
    match maybe_data {
        None => Err(CoreError::FileNonexistent),
        Some(data) => serde_json::from_slice(&data).map_err(core_err_unexpected),
    }
}

pub fn maybe_get(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<EncryptedDocument>, CoreError> {
    let maybe_data: Option<Vec<u8>> =
        local_storage::read(config, namespace(source), id.to_string().as_str())?;
    match maybe_data {
        None => Ok(None),
        Some(data) => serde_json::from_slice(&data)
            .map(Some)
            .map_err(core_err_unexpected),
    }
}

pub fn get_all(config: &Config, source: RepoSource) -> Result<Vec<EncryptedDocument>, CoreError> {
    Ok(
        local_storage::dump::<_, Vec<u8>>(config, namespace(source))?
            .into_iter()
            .map(|s| serde_json::from_slice(s.as_ref()).map_err(core_err_unexpected))
            .collect::<Result<Vec<EncryptedDocument>, CoreError>>()?
            .into_iter()
            .collect(),
    )
}

pub fn delete(config: &Config, source: RepoSource, id: Uuid) -> Result<(), CoreError> {
    local_storage::delete(config, namespace(source), id.to_string().as_str())
}

pub fn delete_all(config: &Config, source: RepoSource) -> Result<(), CoreError> {
    local_storage::delete_all(config, namespace(source))
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::{
        model::{repo::RepoSource, state::temp_config},
        repo::document_repo,
    };
    use lockbook_models::crypto::*;

    #[test]
    fn update_document() {
        let test_document = EncryptedDocument::new("something", "nonce1");

        let config = temp_config();

        let document_id = Uuid::new_v4();

        document_repo::insert(&config, RepoSource::Local, document_id, &test_document).unwrap();

        let document = document_repo::get(&config, RepoSource::Local, document_id).unwrap();
        assert_eq!(document, EncryptedDocument::new("something", "nonce1"),);

        document_repo::insert(
            &config,
            RepoSource::Local,
            document_id,
            &EncryptedDocument::new("updated", "nonce2"),
        )
        .unwrap();

        let file_updated = document_repo::get(&config, RepoSource::Local, document_id).unwrap();

        assert_eq!(file_updated, EncryptedDocument::new("updated", "nonce2"));
    }
}
