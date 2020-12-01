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
        let folder_id = Uuid::new_v4();
        let folder_key = AesImpl::generate_key();
        let version = ClientImpl::new_account(
            &account.api_url,
            &account.username,
            &sign(&account),
            account.keys.to_public_key(),
            folder_id,
            FolderAccessInfo {
                folder_id: folder_id,
                access_key: aes_key(&folder_key, &folder_key),
            },
            rsa_key(&account.keys.to_public_key(), &folder_key),
        )
        .unwrap();

        // moving root into itself
        assert_matches!(
            ClientImpl::move_folder(
                &account.api_url,
                &account.username,
                &sign(&account),
                folder_id,
                version,
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &folder_key),
                },
            ),
            Err(ApiError::<MoveFolderError>::Api(
                MoveFolderError::CannotMoveRoot
            ))
        );
    }

    #[test]
    fn move_folder_into_itself() {
        // new account
        let account = generate_account();
        let folder_id = Uuid::new_v4();
        let folder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::new_account(
                &account.api_url,
                &account.username,
                &sign(&account),
                account.keys.to_public_key(),
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &folder_key),
                },
                rsa_key(&account.keys.to_public_key(), &folder_key)
            ),
            Ok(_)
        );

        // create folder to move itself into
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AesImpl::generate_key();
        let version = ClientImpl::create_folder(
            &account.api_url,
            &account.username,
            &sign(&account),
            subfolder_id,
            &random_filename(),
            folder_id,
            FolderAccessInfo {
                folder_id: subfolder_id,
                access_key: aes_key(&folder_key, &subfolder_key),
            },
        )
        .unwrap();

        assert_matches!(
            ClientImpl::move_folder(
                &account.api_url,
                &account.username,
                &sign(&account),
                subfolder_id,
                version,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_key(&subfolder_key, &subfolder_key),
                }
            ),
            Err(ApiError::<MoveFolderError>::Api(
                MoveFolderError::CannotMoveIntoDescendant
            ))
        );
    }
}
