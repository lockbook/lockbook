#[cfg(test)]
mod import_export_file_tests {
    use rand::Rng;
    use uuid::Uuid;

    use lockbook_core::model::state::temp_config;
    use lockbook_core::service::import_export_service::ImportExportFileInfo;
    use lockbook_core::service::test_utils::generate_account;
    use lockbook_core::{
        create_account, create_file, create_file_at_path, export_file, get_file_by_path, get_root,
        import_file, write_document,
    };
    use lockbook_models::file_metadata::FileType;

    #[test]
    fn import_file_successfully() {
        // new account
        let config = temp_config();
        let generated_account = generate_account();
        create_account(
            &config,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let tmp_path = tmp.path().to_path_buf();

        // generating document in /tmp/
        let name = Uuid::new_v4().to_string();
        let doc_path = tmp_path.join(&name);

        std::fs::write(&doc_path, rand::thread_rng().gen::<[u8; 32]>()).unwrap();

        let root = get_root(&config).unwrap();

        let f = move |info: ImportExportFileInfo| {
            // only checking if the disk path exists since a lockbook folder that has children won't be created until its first child is
            assert!(info.disk_path.exists());
        };

        import_file(&config, doc_path, root.id, Some(Box::new(f.clone()))).unwrap();

        get_file_by_path(&config, &format!("/{}/{}", root.decrypted_name, name)).unwrap();

        // generating folder with a document in /tmp/
        let parent_name = Uuid::new_v4().to_string();
        let parent_path = tmp_path.join(&parent_name);

        std::fs::create_dir(&parent_path).unwrap();

        let child_name = Uuid::new_v4().to_string();
        let child_path = parent_path.join(&child_name);

        std::fs::write(&child_path, rand::thread_rng().gen::<[u8; 32]>()).unwrap();

        import_file(&config, parent_path, root.id, Some(Box::new(f.clone()))).unwrap();

        get_file_by_path(
            &config,
            &format!("/{}/{}/{}", root.decrypted_name, parent_name, child_name),
        )
        .unwrap();
    }

    #[test]
    fn export_file_successfully() {
        // new account
        let config = temp_config();
        let generated_account = generate_account();

        create_account(
            &config,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let root = get_root(&config).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let tmp_path = tmp.path().to_path_buf();

        // generating document in lockbook
        let name = Uuid::new_v4().to_string();
        let file = create_file(&config, &name, root.id, FileType::Document).unwrap();
        write_document(&config, file.id, &rand::thread_rng().gen::<[u8; 32]>()).unwrap();

        let config_copy = config.clone();
        let export_progress = move |info: ImportExportFileInfo| {
            get_file_by_path(&config_copy, &info.lockbook_path).unwrap();
            assert!(info.disk_path.exists());
        };
        export_file(
            &config,
            file.id,
            tmp_path.clone(),
            false,
            Some(Box::new(export_progress.clone())),
        )
        .unwrap();

        assert!(tmp_path.join(file.decrypted_name).exists());

        // generating folder with a document in lockbook
        let parent_name = Uuid::new_v4().to_string();
        let child_name = Uuid::new_v4().to_string();
        let child = create_file_at_path(
            &config,
            &format!("/{}/{}/{}", root.decrypted_name, parent_name, child_name),
        )
        .unwrap();

        write_document(&config, child.id, &rand::thread_rng().gen::<[u8; 32]>()).unwrap();

        export_file(
            &config,
            child.parent,
            tmp_path.clone(),
            false,
            Some(Box::new(export_progress.clone())),
        )
        .unwrap();

        assert!(tmp_path.join(parent_name).join(child_name).exists());
    }
}
