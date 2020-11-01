mod integration_test;

#[cfg(test)]
mod get_usage_tests {
    use crate::integration_test::{generate_account, random_filename, test_config};
    use lockbook_core::model::file_metadata::FileType;
    use lockbook_core::{
        create_account, create_file, get_root, get_usage, sync_all, write_document,
    };

    #[test]
    fn report_usage() {
        let config = &test_config();
        let generated_account = generate_account();
        create_account(
            config,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let root = get_root(config).unwrap();

        let file = create_file(config, &random_filename(), root.id, FileType::Document).unwrap();
        write_document(config, file.id, "0000000000".as_bytes()).unwrap();

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
