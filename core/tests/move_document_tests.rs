mod integration_test;

#[cfg(test)]
mod move_document_tests {
    use crate::assert_matches;
    use crate::integration_test::{
        aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_core::client::{ApiError, Client};
    use lockbook_core::model::api::*;
    use lockbook_core::model::file_metadata::FileType;
    use lockbook_core::DefaultClient;

    #[test]
    fn move_document() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // create folder to move document to
        let (folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &folder,
                aes_encrypt(&folder_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // move document
        doc.parent = folder.id;
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            MoveDocumentRequest::new(&account.username, &doc),
        )
        .unwrap();
    }

    #[test]
    fn move_document_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        // create folder to move document to
        let (folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &folder,
                aes_encrypt(&folder_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // move document that wasn't created
        let (mut doc, _) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.parent = folder.id;
        let result = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            MoveDocumentRequest::new(&account.username, &doc),
        );

        // move document that wasn't created
        assert_matches!(
            result,
            Err(ApiError::<MoveDocumentError>::Api(
                MoveDocumentError::DocumentNotFound
            ))
        );
    }

    #[test]
    fn move_document_parent_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // move document to folder that was never created
        let (folder, _) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        doc.parent = folder.id;
        let result = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            MoveDocumentRequest::new(&account.username, &doc),
        );
        assert_matches!(
            result,
            Err(ApiError::<MoveDocumentError>::Api(
                MoveDocumentError::ParentNotFound
            ))
        );
    }

    #[test]
    fn move_document_deleted() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // create folder to move document to
        let (folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &folder,
                aes_encrypt(&folder_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // delete document
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            DeleteDocumentRequest {
                username: account.username.clone(),
                id: doc.id,
                old_metadata_version: doc.metadata_version,
            },
        )
        .unwrap();

        // move deleted document
        doc.parent = folder.id;
        let result = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            MoveDocumentRequest::new(&account.username, &doc),
        );
        assert_matches!(
            result,
            Err(ApiError::<MoveDocumentError>::Api(
                MoveDocumentError::DocumentDeleted
            ))
        );
    }

    #[test]
    fn move_document_conflict() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // create folder to move document to
        let (folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &folder,
                aes_encrypt(&folder_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // move document
        doc.parent = folder.id;
        doc.metadata_version -= 1;
        let result = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            MoveDocumentRequest::new(&account.username, &doc),
        );
        assert_matches!(
            result,
            Err(ApiError::<MoveDocumentError>::Api(
                MoveDocumentError::EditConflict
            ))
        );
    }

    #[test]
    fn move_document_path_taken() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            NewAccountRequest::new(&account, &root),
        )
        .unwrap();

        // create document
        let (mut doc, doc_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // create folder to move document to
        let (folder, folder_key) =
            generate_file_metadata(&account, &root, &root_key, FileType::Folder);
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &folder,
                aes_encrypt(&folder_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // create document
        let (mut doc2, _) =
            generate_file_metadata(&account, &folder, &folder_key, FileType::Document);
        doc2.name = doc.name.clone();
        DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // move document
        doc.parent = folder.id;
        doc.metadata_version -= 1;
        let result = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            MoveDocumentRequest::new(&account.username, &doc),
        );
        assert_matches!(
            result,
            Err(ApiError::<MoveDocumentError>::Api(
                MoveDocumentError::DocumentPathTaken
            ))
        );
    }
}
