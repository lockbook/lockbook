mod integration_test;

#[cfg(test)]
mod import_export_file_tests {
    use lockbook_core::model::state::temp_config;
    use lockbook_core::service::import_export_service::ImportExportFileInfo;
    use lockbook_core::service::test_utils::generate_account;
    use lockbook_core::{
        create_account, create_file, create_file_at_path, export_file, get_file_by_path, get_root,
        import_file, write_document,
    };
    use lockbook_models::file_metadata::FileType;
    use rand::Rng;
    use uuid::Uuid;

    #[test]
    fn import_file_successfully() {
        // new account
        let config = &temp_config();
        let generated_account = generate_account();
        create_account(
            config,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let root = get_root(config).unwrap();

        let tmp_dir = tempfile::tempdir().unwrap().path().to_path_buf();

        let config_copy = config.clone();
        let f = move |info: ImportExportFileInfo| {
            get_file_by_path(&config_copy, info.lockbook_path.as_str()).unwrap();
            assert!(info.disk_path.exists());
        };

        // generating document in /tmp/
        let name = Uuid::new_v4().to_string();
        let doc_path = tmp_dir.join(&name);

        std::fs::write(&doc_path, rand::thread_rng().gen::<[u8; 32]>()).unwrap();

        import_file(&config, root.id, doc_path, false, Some(Box::new(f.clone()))).unwrap();

        get_file_by_path(&config, format!("/{}/{}", root.name, name).as_str()).unwrap();

        // generating folder with a document in /tmp/
        let parent_name = Uuid::new_v4().to_string();
        let child_name = Uuid::new_v4().to_string();
        let child_path = tmp_dir.join(&parent_name).join(&child_name);

        std::fs::create_dir_all(&child_path).unwrap();
        std::fs::write(&child_path, rand::thread_rng().gen::<[u8; 32]>()).unwrap();

        import_file(
            &config,
            root.id,
            child_path,
            false,
            Some(Box::new(f.clone())),
        )
        .unwrap();

        get_file_by_path(
            &config,
            format!("/{}/{}/{}", root.name, parent_name, child_name).as_str(),
        )
        .unwrap();
    }

    #[test]
    fn export_file_successfully() {
        // new account
        let config = &temp_config();
        let generated_account = generate_account();

        create_account(
            config,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let root = get_root(config).unwrap();

        let tmp_dir = tempfile::tempdir().unwrap().path().to_path_buf();

        // generating document in lockbook
        let name = Uuid::new_v4().to_string();
        let file = create_file(&config, name.as_str(), root.id, FileType::Document).unwrap();
        write_document(&config, file.id, &rand::thread_rng().gen::<[u8; 32]>()).unwrap();

        export_file(&config, file.id, tmp_dir.clone(), false, None).unwrap();

        assert!(tmp_dir.join(file.name).exists());

        // generating folder with a document in lockbook
        let parent_name = Uuid::new_v4().to_string();
        let child_name = Uuid::new_v4().to_string();
        let child = create_file_at_path(
            &config,
            &format!("/{}/{}/{}", root.name, parent_name, child_name),
        )
        .unwrap();

        write_document(&config, child.id, &rand::thread_rng().gen::<[u8; 32]>()).unwrap();

        assert!(tmp_dir.join(parent_name).join(child_name).exists());
    }
}
