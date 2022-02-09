use uuid::Uuid;

use lockbook_models::crypto::*;

use crate::model::errors::core_err_unexpected;
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::local_storage;
use crate::CoreError;

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
        bincode::serialize(document).map_err(core_err_unexpected)?,
    )
}

pub fn get(config: &Config, source: RepoSource, id: Uuid) -> Result<EncryptedDocument, CoreError> {
    let maybe_data: Option<Vec<u8>> =
        local_storage::read(config, namespace(source), id.to_string().as_str())?;
    match maybe_data {
        None => Err(CoreError::FileNonexistent),
        Some(data) => bincode::deserialize(&data).map_err(core_err_unexpected),
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
        Some(data) => bincode::deserialize(&data)
            .map(Some)
            .map_err(core_err_unexpected),
    }
}

pub fn get_all(config: &Config, source: RepoSource) -> Result<Vec<EncryptedDocument>, CoreError> {
    Ok(local_storage::dump::<_, Vec<u8>>(config, namespace(source))?
        .into_iter()
        .map(|s| bincode::deserialize(s.as_ref()).map_err(core_err_unexpected))
        .collect::<Result<Vec<EncryptedDocument>, CoreError>>()?
        .into_iter()
        .collect())
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

    use lockbook_crypto::symkey;
    use lockbook_models::crypto::AESEncrypted;

    use crate::model::repo::RepoSource;
    use crate::model::state::temp_config;
    use crate::repo::document_repo;
    use crate::service::test_utils;

    #[test]
    fn get() {
        let config = &temp_config();

        let id = Uuid::new_v4();
        let result = document_repo::get(config, RepoSource::Local, id);

        assert!(result.is_err());
    }

    #[test]
    fn maybe_get() {
        let config = &temp_config();

        let id = Uuid::new_v4();
        let result = document_repo::maybe_get(config, RepoSource::Local, id).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn insert_get() {
        let config = &temp_config();
        let key = &symkey::generate_key();

        let (id, document) =
            (Uuid::new_v4(), test_utils::aes_encrypt(key, &String::from("document").into_bytes()));
        document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
        let result = document_repo::get(config, RepoSource::Local, id).unwrap();

        assert_eq!(result, document);
    }

    #[test]
    fn insert_get_different_source() {
        let config = &temp_config();
        let key = &symkey::generate_key();

        let (id, document) =
            (Uuid::new_v4(), test_utils::aes_encrypt(key, &String::from("document").into_bytes()));
        document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
        let result = document_repo::maybe_get(config, RepoSource::Base, id).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn insert_get_overwrite_different_source() {
        let config = &temp_config();
        let key = &symkey::generate_key();

        let (id, document) =
            (Uuid::new_v4(), test_utils::aes_encrypt(key, &String::from("document").into_bytes()));
        document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
        let (id_2, document_2) = (
            Uuid::new_v4(),
            test_utils::aes_encrypt(key, &String::from("document_2").into_bytes()),
        );
        document_repo::insert(config, RepoSource::Base, id_2, &document_2).unwrap();
        let result = document_repo::get(config, RepoSource::Local, id).unwrap();

        assert_eq!(result, document);
    }

    #[test]
    fn insert_get_all() {
        let config = &temp_config();
        let key = &symkey::generate_key();

        let (id, document) =
            (Uuid::new_v4(), test_utils::aes_encrypt(key, &String::from("document").into_bytes()));
        document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
        let (id_2, document_2) = (
            Uuid::new_v4(),
            test_utils::aes_encrypt(key, &String::from("document_2").into_bytes()),
        );
        document_repo::insert(config, RepoSource::Local, id_2, &document_2).unwrap();
        let (id_3, document_3) = (
            Uuid::new_v4(),
            test_utils::aes_encrypt(key, &String::from("document_3").into_bytes()),
        );
        document_repo::insert(config, RepoSource::Local, id_3, &document_3).unwrap();
        let (id_4, document_4) = (
            Uuid::new_v4(),
            test_utils::aes_encrypt(key, &String::from("document_4").into_bytes()),
        );
        document_repo::insert(config, RepoSource::Local, id_4, &document_4).unwrap();
        let (id_5, document_5) = (
            Uuid::new_v4(),
            test_utils::aes_encrypt(key, &String::from("document_5").into_bytes()),
        );
        document_repo::insert(config, RepoSource::Local, id_5, &document_5).unwrap();
        let result = document_repo::get_all(config, RepoSource::Local).unwrap();

        let mut expectation = vec![
            (id, document),
            (id_2, document_2),
            (id_3, document_3),
            (id_4, document_4),
            (id_5, document_5),
        ];
        expectation.sort_by(|(a, _), (b, _)| a.cmp(&b));
        let expectation = expectation
            .into_iter()
            .map(|(_, d)| d)
            .collect::<Vec<AESEncrypted<Vec<u8>>>>();
        assert_eq!(result, expectation);
    }

    #[test]
    fn insert_get_all_different_source() {
        let config = &temp_config();
        let key = &symkey::generate_key();

        let (id, document) =
            (Uuid::new_v4(), test_utils::aes_encrypt(key, &String::from("document").into_bytes()));
        document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
        let result = document_repo::get_all(config, RepoSource::Base).unwrap();

        assert_eq!(result, Vec::<AESEncrypted<Vec<u8>>>::new());
    }

    #[test]
    fn insert_delete() {
        let config = &temp_config();
        let key = &symkey::generate_key();

        let (id, document) =
            (Uuid::new_v4(), test_utils::aes_encrypt(key, &String::from("document").into_bytes()));
        document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
        document_repo::delete(config, RepoSource::Local, id).unwrap();
        let result = document_repo::maybe_get(config, RepoSource::Local, id).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn insert_delete_all() {
        let config = &temp_config();
        let key = &symkey::generate_key();

        let (id, document) =
            (Uuid::new_v4(), test_utils::aes_encrypt(key, &String::from("document").into_bytes()));
        document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
        document_repo::delete_all(config, RepoSource::Local).unwrap();
        let result = document_repo::get_all(config, RepoSource::Local).unwrap();

        assert_eq!(result, Vec::<AESEncrypted<Vec<u8>>>::new());
    }

    #[test]
    fn insert_delete_all_different_source() {
        let config = &temp_config();
        let key = &symkey::generate_key();

        let (id, document) =
            (Uuid::new_v4(), test_utils::aes_encrypt(key, &String::from("document").into_bytes()));
        document_repo::insert(config, RepoSource::Local, id, &document).unwrap();
        document_repo::delete_all(config, RepoSource::Base).unwrap();
        let result = document_repo::get_all(config, RepoSource::Local).unwrap();

        assert_eq!(result, vec![document]);
    }
}
