mod integration_test;

#[cfg(test)]
mod get_usage_tests {
    use lockbook_core::model::crypto::*;
    use lockbook_core::model::file_metadata::FileType;
    use lockbook_core::model::file_metadata::FileType::Folder;
    use lockbook_core::repo::document_repo::DocumentRepo;
    use lockbook_core::{
        connect_to_db, create_account, create_file, delete_file, get_root, get_usage, init_logger,
        sync_all, write_document, DefaultDocumentRepo,
    };

    use crate::integration_test::{generate_account, random_filename, test_config};
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

        let local_encrypted = {
            let db = connect_to_db(config).unwrap();
            DefaultDocumentRepo::get(&db, file.id).unwrap().content
        };

        assert_eq!(get_usage(config).unwrap()[0].file_id, file.id);
        assert_eq!(get_usage(config).unwrap().len(), 1);
        assert_eq!(
            get_usage(config).unwrap()[0].byte_secs,
            serde_json::to_vec(&local_encrypted).unwrap().len() as u64
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
        write_document(
            config,
            file.id,
            &DecryptedValue {
                secret: "0000000000".to_string(),
            },
        )
        .unwrap();

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
        write_document(
            config,
            file.id,
            &DecryptedValue {
                secret: "0000000000".to_string(),
            },
        )
        .unwrap();
        let file2 = create_file(config, &random_filename(), folder.id, FileType::Document).unwrap();
        write_document(
            config,
            file2.id,
            &DecryptedValue {
                secret: "0000000000".to_string(),
            },
        )
        .unwrap();
        let file3 = create_file(config, &random_filename(), folder.id, FileType::Document).unwrap();
        write_document(
            config,
            file3.id,
            &DecryptedValue {
                secret: "0000000000".to_string(),
            },
        )
        .unwrap();

        sync_all(config).unwrap();
        delete_file(config, folder.id).unwrap();
        sync_all(config).unwrap();

        let local_encrypted = {
            let db = connect_to_db(config).unwrap();
            DefaultDocumentRepo::get(&db, file.id).unwrap().content
        };

        let usages = get_usage(config).unwrap();
        let mut total_usage = 0;
        for usage in usages {
            total_usage += usage.byte_secs;
        }

        assert_eq!(get_usage(config).unwrap().len(), 3);
        assert_eq!(
            total_usage,
            serde_json::to_vec(&local_encrypted).unwrap().len() as u64
        )
    }
}
