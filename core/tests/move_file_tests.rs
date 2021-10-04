mod integration_test;

#[cfg(test)]
mod move_document_tests {
    use lockbook_core::client::ApiError;
    use lockbook_core::service::test_utils::{
        generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_core::{assert_get_updates_required, assert_matches, client};
    use lockbook_models::api::*;
    use lockbook_models::file_metadata::FileMetadataDiff;
    use lockbook_models::file_metadata::FileType;

    #[test]
    fn move_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document and folder
        let (mut doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let (folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new(&doc), FileMetadataDiff::new(&folder)],
            },
        )
        .unwrap();

        // move document
        doc.parent = folder.id;
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)],
            },
        )
        .unwrap();
    }

    #[test]
    fn move_document_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document and folder
        let (folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        let (doc, _doc_key) =
            generate_file_metadata(&account, &folder, &root_key, FileType::Document);
        let result = client::request(
            &account,
            FileMetadataUpsertsRequest {
                // create document as if moving an existing document
                updates: vec![
                    FileMetadataDiff::new_diff(root.id, &doc.name, &doc),
                    FileMetadataDiff::new(&folder),
                ],
            },
        );
        assert_get_updates_required!(result);
    }

    #[test]
    fn move_document_parent_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document and folder, but don't send folder to server
        let (folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        let (doc, _doc_key) =
            generate_file_metadata(&account, &folder, &root_key, FileType::Document);
        let result = client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new(&doc)],
            },
        );
        assert_get_updates_required!(result);
    }

    #[test]
    fn move_document_deleted() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create deleted document and folder
        let (mut doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.deleted = true;
        let (folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new(&doc), FileMetadataDiff::new(&folder)],
            },
        )
        .unwrap();

        // move document
        doc.parent = folder.id;
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)],
            },
        )
        .unwrap();
    }

    #[test]
    fn move_document_conflict() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document and folder
        let (mut doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let (folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new(&doc), FileMetadataDiff::new(&folder)],
            },
        )
        .unwrap();

        // move document
        doc.parent = folder.id;
        let result = client::request(
            &account,
            FileMetadataUpsertsRequest {
                // use incorrect previous parent
                updates: vec![FileMetadataDiff::new_diff(folder.id, &doc.name, &doc)],
            },
        );
        assert_get_updates_required!(result);
    }

    #[test]
    fn move_document_path_taken() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create documents and folder
        let (folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        let (mut doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let (mut doc2, _doc_key2) =
            generate_file_metadata(&account, &folder, &root_key, FileType::Document);
        doc2.name = doc.name.clone();
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![
                    FileMetadataDiff::new(&doc),
                    FileMetadataDiff::new(&doc2),
                    FileMetadataDiff::new(&folder),
                ],
            },
        )
        .unwrap();

        // move document
        doc.parent = folder.id;
        let result = client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)],
            },
        );
        assert_get_updates_required!(result);
    }

    #[test]
    fn move_folder_cannot_move_root() {
        // new account
        let account = generate_account();
        let (mut root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create folder
        let (folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new(&folder)],
            },
        )
        .unwrap();

        // move root
        root.parent = folder.id;
        let result = client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &root.name, &root)],
            },
        );
        assert_get_updates_required!(result);
    }

    #[test]
    fn move_folder_into_itself() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create folder
        let (mut folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new(&folder)],
            },
        )
        .unwrap();

        // move folder into self
        folder.parent = folder.id;
        let result = client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &folder.name, &folder)],
            },
        );
        assert_get_updates_required!(result);
    }

    #[test]
    fn move_folder_into_descendants() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create folders
        let (mut folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        let (folder2, _folder_key2) =
            generate_file_metadata(&account, &folder, &root_key, FileType::Folder);
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![
                    FileMetadataDiff::new(&folder),
                    FileMetadataDiff::new(&folder2),
                ],
            },
        )
        .unwrap();

        // move folder into itself
        folder.parent = folder2.id;
        let result = client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &folder.name, &folder)],
            },
        );
        assert_get_updates_required!(result);
    }

    #[test]
    fn move_document_into_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create documents
        let (mut doc, _doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let (doc2, _doc_key2) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new(&doc), FileMetadataDiff::new(&doc2)],
            },
        )
        .unwrap();

        // move folder into itself
        doc.parent = doc2.id;
        let result = client::request(
            &account,
            FileMetadataUpsertsRequest {
                updates: vec![FileMetadataDiff::new_diff(root.id, &doc.name, &doc)],
            },
        );
        assert_get_updates_required!(result);
    }
}
