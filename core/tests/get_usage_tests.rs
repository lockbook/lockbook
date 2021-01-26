mod integration_test;

#[cfg(test)]
mod get_usage_tests {
    use crate::integration_test::{generate_account, random_filename, test_config};
    use lockbook_core::model::file_metadata::FileType;
    use lockbook_core::model::file_metadata::FileType::Folder;
    use lockbook_core::repo::document_repo::DocumentRepo;
    use lockbook_core::storage::db_provider::Backend;
    use lockbook_core::{
        create_account, create_file, delete_file, get_root, get_usage, get_usage_human_string,
        init_logger, sync_all, write_document, DefaultBackend, DefaultDocumentRepo,
    };
    use std::path::Path;

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

        let local_encrypted = {
            let backend = DefaultBackend::connect_to_db(config).unwrap();
            DefaultDocumentRepo::get(&backend, file.id).unwrap().value
        };

        assert_eq!(get_usage(config).unwrap()[0].file_id, file.id);
        assert_eq!(get_usage(config).unwrap().len(), 1);
        assert_eq!(
            get_usage(config).unwrap()[0].byte_secs,
            local_encrypted.len() as u64
        )
    }

    #[test]
    fn usage_go_back_down_after_delete() {
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
        write_document(config, file.id, &String::from("0000000000").into_bytes()).unwrap();

        sync_all(config).unwrap();
        delete_file(config, file.id).unwrap();
        sync_all(config).unwrap();

        assert_eq!(get_usage(config).unwrap()[0].file_id, file.id);
        assert_eq!(get_usage(config).unwrap().len(), 1);
        assert_eq!(get_usage(config).unwrap()[0].byte_secs, 0)
    }

    #[test]
    fn usage_go_back_down_after_delete_folder() {
        let config = &test_config();
        init_logger(Path::new("/tmp/logs")).unwrap();
        let generated_account = generate_account();
        create_account(
            config,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let root = get_root(config).unwrap();

        let folder = create_file(config, "folder", root.id, Folder).unwrap();
        let file = create_file(config, &random_filename(), root.id, FileType::Document).unwrap();
        write_document(config, file.id, &String::from("0000000000").into_bytes()).unwrap();
        let file2 = create_file(config, &random_filename(), folder.id, FileType::Document).unwrap();
        write_document(config, file2.id, &String::from("0000000000").into_bytes()).unwrap();
        let file3 = create_file(config, &random_filename(), folder.id, FileType::Document).unwrap();
        write_document(config, file3.id, &String::from("0000000000").into_bytes()).unwrap();

        sync_all(config).unwrap();
        delete_file(config, folder.id).unwrap();
        sync_all(config).unwrap();

        let local_encrypted = {
            let backend = DefaultBackend::connect_to_db(config).unwrap();
            DefaultDocumentRepo::get(&backend, file.id).unwrap().value
        };

        let usages = get_usage(config).unwrap();
        let mut total_usage = 0;
        for usage in usages {
            total_usage += usage.byte_secs;
        }

        assert_eq!(get_usage(config).unwrap().len(), 3);
        assert_eq!(total_usage, local_encrypted.len() as u64)
    }

    #[test]
    fn usage_human_string_sanity_check() {
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

        let pre_usage = get_usage_human_string(config, false).unwrap();
        let pre_usage_exact = get_usage_human_string(config, true).unwrap();

        assert_eq!(pre_usage, "0.000 B");
        assert_eq!(pre_usage_exact, "0");

        sync_all(config).unwrap();

        let local_encrypted = {
            let backend = DefaultBackend::connect_to_db(config).unwrap();
            DefaultDocumentRepo::get(&backend, file.id).unwrap().value
        };

        let post_usage = get_usage_human_string(config, false).unwrap();
        let post_usage_exact = get_usage_human_string(config, true).unwrap();

        assert_eq!(post_usage, format!("{}.000 B", local_encrypted.len()));
        assert_eq!(post_usage_exact, local_encrypted.len().to_string());
    }
}
