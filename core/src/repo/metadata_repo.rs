use uuid::Uuid;

use lockbook_models::file_metadata::EncryptedFileMetadata;

use crate::model::errors::core_err_unexpected;
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::local_storage;
use crate::CoreError;

const NAMESPACE_LOCAL: &str = "changed_local_metadata";
const NAMESPACE_BASE: &str = "all_base_metadata";

fn namespace(source: RepoSource) -> &'static str {
    match source {
        RepoSource::Local => NAMESPACE_LOCAL,
        RepoSource::Base => NAMESPACE_BASE,
    }
}

pub fn insert(
    config: &Config,
    source: RepoSource,
    file: &EncryptedFileMetadata,
) -> Result<(), CoreError> {
    local_storage::write(
        config,
        namespace(source),
        file.id.to_string().as_str(),
        serde_json::to_vec(&file).map_err(core_err_unexpected)?,
    )
}

pub fn get(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<EncryptedFileMetadata, CoreError> {
    maybe_get(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<EncryptedFileMetadata>, CoreError> {
    let maybe_bytes: Option<Vec<u8>> =
        local_storage::read(config, namespace(source), id.to_string().as_str())?;
    Ok(match maybe_bytes {
        Some(bytes) => Some(serde_json::from_slice(&bytes).map_err(core_err_unexpected)?),
        None => None,
    })
}

pub fn get_all(
    config: &Config,
    source: RepoSource,
) -> Result<Vec<EncryptedFileMetadata>, CoreError> {
    Ok(
        local_storage::dump::<_, Vec<u8>>(config, namespace(source))?
            .into_iter()
            .map(|s| serde_json::from_slice(s.as_ref()).map_err(core_err_unexpected))
            .collect::<Result<Vec<EncryptedFileMetadata>, CoreError>>()?
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

    use crate::model::repo::RepoSource;
    use crate::model::state::temp_config;
    use crate::repo::metadata_repo;
    use crate::service::test_utils;

    #[test]
    fn get() {
        let config = &temp_config();

        let id = Uuid::new_v4();
        let result = metadata_repo::get(config, RepoSource::Local, id);

        assert!(result.is_err());
    }

    #[test]
    fn maybe_get() {
        let config = &temp_config();

        let id = Uuid::new_v4();
        let result = metadata_repo::maybe_get(config, RepoSource::Local, id).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn insert_get() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        let (metadata, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata).unwrap();
        let result = metadata_repo::get(config, RepoSource::Local, metadata.id).unwrap();

        assert_eq!(result, metadata);
    }

    #[test]
    fn insert_get_different_source() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        let (metadata, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata).unwrap();
        let result = metadata_repo::maybe_get(config, RepoSource::Base, metadata.id).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn insert_get_overwrite_different_source() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        let (metadata, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata).unwrap();
        let (metadata_2, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Base, &metadata_2).unwrap();
        let result = metadata_repo::get(config, RepoSource::Local, metadata.id).unwrap();

        assert_eq!(result, metadata);
    }

    #[test]
    fn insert_get_all() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        let (metadata, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata).unwrap();
        let (metadata_2, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata_2).unwrap();
        let (metadata_3, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata_3).unwrap();
        let (metadata_4, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata_4).unwrap();
        let (metadata_5, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata_5).unwrap();
        let result = metadata_repo::get_all(config, RepoSource::Local).unwrap();

        let mut expectation = vec![metadata, metadata_2, metadata_3, metadata_4, metadata_5];
        expectation.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(result, expectation);
    }

    #[test]
    fn insert_get_all_different_source() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        let (metadata, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata).unwrap();
        let result = metadata_repo::get_all(config, RepoSource::Base).unwrap();

        assert_eq!(result, Vec::new());
    }

    #[test]
    fn insert_delete() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        let (metadata, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata).unwrap();
        metadata_repo::delete(config, RepoSource::Local, metadata.id).unwrap();
        let result = metadata_repo::maybe_get(config, RepoSource::Local, metadata.id).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn insert_delete_all() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        let (metadata, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata).unwrap();
        metadata_repo::delete_all(config, RepoSource::Local).unwrap();
        let result = metadata_repo::get_all(config, RepoSource::Local).unwrap();

        assert_eq!(result, Vec::new());
    }

    #[test]
    fn insert_delete_all_different_source() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        let (metadata, _) = test_utils::generate_root_metadata(&account);
        metadata_repo::insert(config, RepoSource::Local, &metadata).unwrap();
        metadata_repo::delete_all(config, RepoSource::Base).unwrap();
        let result = metadata_repo::get_all(config, RepoSource::Local).unwrap();

        assert_eq!(result, vec![metadata]);
    }
}
