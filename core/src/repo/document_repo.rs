use uuid::Uuid;

use lockbook_shared::crypto::*;

use crate::model::errors::core_err_unexpected;
use crate::model::repo::RepoSource;
use crate::repo::local_storage;
use crate::{Config, CoreError};

const NAMESPACE_LOCAL: &str = "changed_local_documents";
const NAMESPACE_BASE: &str = "all_base_documents";

pub fn namespace(source: RepoSource) -> &'static str {
    match source {
        RepoSource::Local => NAMESPACE_LOCAL,
        RepoSource::Base => NAMESPACE_BASE,
    }
}

#[instrument(level = "debug", skip(config, document), err(Debug))]
pub fn insert(
    config: &Config, source: RepoSource, id: Uuid, document: &EncryptedDocument,
) -> Result<(), CoreError> {
    local_storage::write(
        config,
        namespace(source),
        id.to_string().as_str(),
        bincode::serialize(document).map_err(core_err_unexpected)?,
    )
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn get(config: &Config, source: RepoSource, id: Uuid) -> Result<EncryptedDocument, CoreError> {
    let maybe_data: Option<Vec<u8>> =
        local_storage::read(config, namespace(source), id.to_string().as_str())?;
    match maybe_data {
        None => Err(CoreError::FileNonexistent),
        Some(data) => bincode::deserialize(&data).map_err(core_err_unexpected),
    }
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn maybe_get(
    config: &Config, source: RepoSource, id: &Uuid,
) -> Result<Option<EncryptedDocument>, CoreError> {
    let maybe_data: Option<Vec<u8>> =
        local_storage::read(config, namespace(source), id.to_string().as_str())?;
    match maybe_data {
        None => Ok(None),
        Some(data) => bincode::deserialize(&data)
            .map(Some)
            .map_err(core_err_unexpected),
    }
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn delete(config: &Config, source: RepoSource, id: Uuid) -> Result<(), CoreError> {
    local_storage::delete(config, namespace(source), id.to_string().as_str())
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn delete_all(config: &Config, source: RepoSource) -> Result<(), CoreError> {
    local_storage::delete_all(config, namespace(source))
}
