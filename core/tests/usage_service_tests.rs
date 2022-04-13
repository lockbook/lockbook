#[cfg(test)]
mod unit_tests {
    use lockbook_models::file_metadata::FileType;

    use crate::pure_functions::files;
    use crate::service::file_service;
    use crate::{
        model::{repo::RepoSource, state::temp_config},
        repo::account_repo,
        service::{
            test_utils,
            usage_service::{self, UsageItemMetric},
        },
    };

    #[test]
    fn bytes_to_human_kb() {
        assert_eq!(usage_service::bytes_to_human(2000), "2 KB".to_string());
    }

    #[test]
    fn bytes_to_human_mb() {
        assert_eq!(usage_service::bytes_to_human(2000000), "2 MB".to_string());
    }

    #[test]
    fn bytes_to_human_gb() {
        assert_eq!(usage_service::bytes_to_human(2000000000), "2 GB".to_string());
    }

    #[test]
    fn get_uncompressed_usage_no_documents() {
        let config = &temp_config();
        let account = test_utils::generate_account();

        account_repo::insert(config, &account).unwrap();

        assert_eq!(
            usage_service::get_uncompressed_usage(config).unwrap(),
            UsageItemMetric { exact: 0, readable: "0 B".to_string() }
        );
    }

    #[test]
    fn get_uncompressed_usage_empty_document() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"").unwrap();

        assert_eq!(
            usage_service::get_uncompressed_usage(config).unwrap(),
            UsageItemMetric { exact: 0, readable: "0 B".to_string() }
        );
    }

    #[test]
    fn get_uncompressed_usage_one_document() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"0123456789").unwrap();

        assert_eq!(
            usage_service::get_uncompressed_usage(config).unwrap(),
            UsageItemMetric { exact: 10, readable: "10 B".to_string() }
        );
    }

    #[test]
    fn get_uncompressed_usage_multiple_documents() {
        let config = &temp_config();
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());
        let document2 =
            files::create(FileType::Document, root.id, "document 2", &account.public_key());

        account_repo::insert(config, &account).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &root).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document).unwrap();
        file_service::insert_metadatum(config, RepoSource::Base, &document2).unwrap();
        file_service::insert_document(config, RepoSource::Base, &document, b"01234").unwrap();
        file_service::insert_document(config, RepoSource::Base, &document2, b"56789").unwrap();

        assert_eq!(
            usage_service::get_uncompressed_usage(config).unwrap(),
            UsageItemMetric { exact: 10, readable: "10 B".to_string() }
        );
    }
}
