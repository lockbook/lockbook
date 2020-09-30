mod integration_test;

#[cfg(test)]
mod get_usage_tests {
    use crate::integration_test::{random_filename, random_username, test_config};
    use lockbook_core::model::crypto::*;
    use lockbook_core::{
        create_account, create_file, get_root, get_usage, sync_all, write_document,
    };

    use lockbook_core::model::file_metadata::FileType;

    #[test]
    fn report_usage() {
        let config = &test_config();
        create_account(config, &random_username()).unwrap();
        let root = get_root(config).unwrap();

        let file = create_file(config, &random_filename(), root.id, FileType::Document).unwrap();
        write_document(
            config,
            file.id,
            &DecryptedValue {
                secret: "0000000000".to_string(),
            },
        )
        .unwrap();

        assert!(
            get_usage(config).unwrap().is_empty(),
            "Returned non-empty usage!"
        );

        sync_all(config).unwrap();

        assert!(
            !get_usage(config).unwrap().is_empty(),
            "Returned empty usage after file sync!"
        );
    }
}
