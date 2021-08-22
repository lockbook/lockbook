use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::local_storage;
use crate::{core_err_unexpected, CoreError};
use uuid::Uuid;

const NAMESPACE_LOCAL: &str = "changed_local_document_digests";
const NAMESPACE_BASE: &str = "all_base_document_digests";

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
    digest: &[u8],
) -> Result<(), CoreError> {
    local_storage::write(config, namespace(source), id.to_string().as_str(), digest)
}

pub fn get(config: &Config, source: RepoSource, id: Uuid) -> Result<Vec<u8>, CoreError> {
    maybe_get(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<Vec<u8>>, CoreError> {
    local_storage::read(config, namespace(source), id.to_string().as_str())
}

pub fn get_all(config: &Config, source: RepoSource) -> Result<Vec<Vec<u8>>, CoreError> {
    Ok(
        local_storage::dump::<_, Vec<u8>>(config, namespace(source))?
            .into_iter()
            .map(|s| serde_json::from_slice(s.as_ref()).map_err(core_err_unexpected))
            .collect::<Result<Vec<Vec<u8>>, CoreError>>()?
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
