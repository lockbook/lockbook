mod integration_test;

#[cfg(test)]
mod rename_document_tests {
    use crate::assert_matches;
    use crate::integration_test::{
        aes_encrypt, generate_account, generate_file_metadata, generate_root_metadata,
    };
    use lockbook_core::client::{ApiError, Client};
    use lockbook_core::model::api::*;
    use lockbook_core::model::file_metadata::FileType;
    use lockbook_core::DefaultClient;

    #[test]
    fn rename_document() {
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

        // rename document
        doc.name = String::from("new name");
        DefaultClient::request(&account, RenameDocumentRequest::new(&doc)).unwrap();
    }

    #[test]
    fn rename_document_not_found() {
        // new account
        let account = generate_account();
        let (root, root_key) = generate_root_metadata(&account);
        DefaultClient::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // rename document that wasn't created
        let (mut doc, _) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        doc.name = String::from("new name");
        let result = DefaultClient::request(&account, RenameDocumentRequest::new(&doc));
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

        // delete document
        DefaultClient::request(&account, DeleteDocumentRequest { id: doc.id }).unwrap();

        // rename document
        doc.name = String::from("new name");
        let result = DefaultClient::request(&account, RenameDocumentRequest::new(&doc));
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

        // rename document
        doc.name = String::from("new name");
        doc.metadata_version -= 1;
        let result = DefaultClient::request(&account, RenameDocumentRequest::new(&doc));
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

        // create document in same folder
        let (doc2, _) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        DefaultClient::request(
            &account,
            CreateDocumentRequest::new(
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap();

        // rename first document to same name as second
        doc.name = doc2.name;
        let result = DefaultClient::request(&account, RenameDocumentRequest::new(&doc));
        assert_matches!(
            result,
            Err(ApiError::<RenameDocumentError>::Endpoint(
                RenameDocumentError::DocumentPathTaken
            ))
        );
    }
}
