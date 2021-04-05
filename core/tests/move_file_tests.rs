mod integration_test;

#[cfg(test)]
mod move_document_tests {
    use crate::assert_matches;
    use crate::integration_test::{
        aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_core::client::{ApiError, Client};
    use lockbook_core::DefaultClient;
    use lockbook_models::api::*;
    use lockbook_models::file_metadata::FileType;

    #[test]
    fn move_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.metadata_version = DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // create folder to move document to
        let (mut folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        folder.metadata_version = DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &folder,
                aes_encrypt(&folder_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // move document
        doc.parent = folder.id;
        DefaultClient::request(&account, MoveDocumentRequest::new(&doc)).unwrap();
    }

    #[test]
    fn move_document_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create folder to move document to
        let (folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &folder,
                aes_encrypt(&folder_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // move document that wasn't created
        let (mut doc, _) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.parent = folder.id;
        let result = DefaultClient::request(&account, MoveDocumentRequest::new(&doc));

        // move document that wasn't created
        assert_matches!(
            result,
            Err(ApiError::<MoveDocumentError>::Endpoint(
                MoveDocumentError::DocumentNotFound
            ))
        );
    }

    #[test]
    fn move_document_parent_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.metadata_version = DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // move document to folder that was never created
        let (folder, _) = generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        doc.parent = folder.id;
        let result = DefaultClient::request(&account, MoveDocumentRequest::new(&doc));
        assert_matches!(
            result,
            Err(ApiError::<MoveDocumentError>::Endpoint(
                MoveDocumentError::ParentNotFound
            ))
        );
    }

    #[test]
    fn move_document_deleted() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // create folder to move document to
        let (folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &folder,
                aes_encrypt(&folder_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // delete document
        DefaultClient::request(&account, DeleteDocumentRequest { id: doc.id }).unwrap();

        // move deleted document
        doc.parent = folder.id;
        let result = DefaultClient::request(&account, MoveDocumentRequest::new(&doc));
        assert_matches!(
            result,
            Err(ApiError::<MoveDocumentError>::Endpoint(
                MoveDocumentError::DocumentDeleted
            ))
        );
    }

    #[test]
    fn move_document_conflict() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.metadata_version = DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // create folder to move document to
        let (folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &folder,
                aes_encrypt(&folder_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // move document
        doc.parent = folder.id;
        doc.metadata_version -= 1;
        let result = DefaultClient::request(&account, MoveDocumentRequest::new(&doc));
        assert_matches!(
            result,
            Err(ApiError::<MoveDocumentError>::Endpoint(
                MoveDocumentError::EditConflict
            ))
        );
    }

    #[test]
    fn move_document_path_taken() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.metadata_version = DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // create folder to move document to
        let (mut folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        folder.metadata_version = DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &folder,
                aes_encrypt(&folder_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // create document
        let (mut doc2, _) =
            generate_file_metadata(&account, &folder, &folder_key, FileType::Document);
        doc2.name = doc.name.clone();
        doc2.metadata_version = DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &doc2,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // move document
        doc.parent = folder.id;
        let result = DefaultClient::request(&account, MoveDocumentRequest::new(&doc));
        assert_matches!(
            result,
            Err(ApiError::<MoveDocumentError>::Endpoint(
                MoveDocumentError::DocumentPathTaken
            ))
        );
    }

    #[test]
    fn move_folder_cannot_move_root() {
        // new account
        let account = generate_account();
        let (mut root, root_key) = generate_root_metadata(&account);
        root.metadata_version =
            DefaultClient::request(&account, NewAccountRequest::new(&account, &root))
                .unwrap()
                .folder_metadata_version;

        // create folder that will be moved into itself
        let (mut folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        folder.metadata_version =
            DefaultClient::request(&account, CreateFolderRequest::new(&folder))
                .unwrap()
                .new_metadata_version;

        // move root into its child
        root.parent = folder.id;
        let result = DefaultClient::request(&account, MoveFolderRequest::new(&root));
        assert_matches!(
            result,
            Err(ApiError::<MoveFolderError>::Endpoint(
                MoveFolderError::CannotMoveRoot
            ))
        );
    }

    #[test]
    fn move_folder_into_itself() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create folder that will be moved into itself
        let (mut folder, _folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        folder.metadata_version =
            DefaultClient::request(&account, CreateFolderRequest::new(&folder))
                .unwrap()
                .new_metadata_version;

        // move folder into itself
        folder.parent = folder.id;
        let result = DefaultClient::request(&account, MoveFolderRequest::new(&folder));
        assert_matches!(
            result,
            Err(ApiError::<MoveFolderError>::Endpoint(
                MoveFolderError::CannotMoveIntoDescendant
            ))
        );
    }

    #[test]
    fn move_folder_into_descendants() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // create folder that will be moved
        let (mut folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        folder.metadata_version =
            DefaultClient::request(&account, CreateFolderRequest::new(&folder))
                .unwrap()
                .new_metadata_version;

        // create folder to move parent to
        let (mut folder2, folder_key2) =
            generate_file_metadata(&account, &folder, &folder_key, FileType::Folder);
        folder2.metadata_version =
            DefaultClient::request(&account, CreateFolderRequest::new(&folder2))
                .unwrap()
                .new_metadata_version;

        // create folder to move parent to
        let (mut folder3, _folder_key3) =
            generate_file_metadata(&account, &folder2, &folder_key2, FileType::Folder);
        folder3.metadata_version =
            DefaultClient::request(&account, CreateFolderRequest::new(&folder3))
                .unwrap()
                .new_metadata_version;

        // move folder into itself
        folder.parent = folder3.id;
        let result = DefaultClient::request(&account, MoveFolderRequest::new(&folder));
        assert_matches!(
            result,
            Err(ApiError::<MoveFolderError>::Endpoint(
                MoveFolderError::CannotMoveIntoDescendant
            ))
        );
    }
}
