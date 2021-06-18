mod integration_test;

#[cfg(test)]
mod rename_document_tests {
    use lockbook_core::assert_matches;
    use lockbook_core::client;
    use lockbook_core::client::ApiError;
    use lockbook_core::service::test_utils::{
        aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
        random_filename,
    };
    use lockbook_models::api::*;
    use lockbook_models::file_metadata::FileType;

    #[test]
    fn rename_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.metadata_version = client::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // rename document
        doc.name = random_filename();
        client::request(&account, RenameDocumentRequest::new(&doc)).unwrap();
    }

    #[test]
    fn rename_document_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // rename document that wasn't created
        let (mut doc, _) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.name = random_filename();
        let result = client::request(&account, RenameDocumentRequest::new(&doc));
        assert_matches!(
            result,
            Err(ApiError::<RenameDocumentError>::Endpoint(
                RenameDocumentError::DocumentNotFound
            ))
        );
    }

    #[test]
    fn rename_document_deleted() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        client::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // delete document
        client::request(&account, DeleteDocumentRequest { id: doc.id }).unwrap();

        // rename document
        doc.name = random_filename();
        let result = client::request(&account, RenameDocumentRequest::new(&doc));
        assert_matches!(
            result,
            Err(ApiError::<RenameDocumentError>::Endpoint(
                RenameDocumentError::DocumentDeleted
            ))
        );
    }

    #[test]
    fn rename_document_conflict() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.metadata_version = client::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // rename document
        doc.name = random_filename();
        doc.metadata_version -= 1;
        let result = client::request(&account, RenameDocumentRequest::new(&doc));
        assert_matches!(
            result,
            Err(ApiError::<RenameDocumentError>::Endpoint(
                RenameDocumentError::EditConflict
            ))
        );
    }

    #[test]
    fn rename_document_path_taken() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        client::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.metadata_version = client::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // create document in same folder
        let (doc2, _) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        client::request(
            &account,
            CreateDocumentRequest::new(
                &doc2,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // rename first document to same name as second
        doc.name = doc2.name;
        let result = client::request(&account, RenameDocumentRequest::new(&doc));
        assert_matches!(
            result,
            Err(ApiError::<RenameDocumentError>::Endpoint(
                RenameDocumentError::DocumentPathTaken
            ))
        );
    }

    #[test]
    fn rename_folder_cannot_rename_root() {
        // new account
        let account = generate_account();
        let (mut root, _root_key) = generate_root_metadata(&account);
        root.metadata_version = client::request(&account, NewAccountRequest::new(&account, &root))
            .unwrap()
            .folder_metadata_version;

        // rename root
        let result = client::request(&account, RenameFolderRequest::new(&root));
        assert_matches!(
            result,
            Err(ApiError::<RenameFolderError>::Endpoint(
                RenameFolderError::CannotRenameRoot
            ))
        );
    }
}
