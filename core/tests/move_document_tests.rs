mod integration_test;

#[cfg(test)]
mod move_document_tests {
    use crate::assert_matches;
    use crate::integration_test::{aes_encrypt, generate_account, random_filename, rsa_encrypt};
    use lockbook_core::client::{ApiError, Client, ClientImpl};
    use lockbook_core::model::api::*;
    use lockbook_core::model::crypto::*;
    use lockbook_core::model::file_metadata::FileType;
    use lockbook_core::service::clock_service::ClockImpl;
    use lockbook_core::service::code_version_service::CodeVersionImpl;
    use lockbook_core::service::crypto_service::RSAImpl;
    use lockbook_core::service::crypto_service::{AESImpl, SymmetricCryptoService};
    use uuid::Uuid;

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
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let version = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // create folder to move document to
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AESImpl::generate_key();

        assert_matches!(
            DefaultClient::create_folder(
                &account.api_url,
                &account.username,
                subfolder_id,
                &random_filename(),
                folder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_encrypt(&folder_key, &subfolder_key),
                },
            ),
            Ok(_)
        );

        // move document
        assert_matches!(
            DefaultClient::move_document(
                &account.api_url,
                &account.username,
                doc_id,
                version,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_encrypt(&folder_key, &subfolder_key),
                }
            ),
            Ok(_)
        );
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

        // move document that wasn't created
        assert_matches!(
            DefaultClient::move_document(
                &account.api_url,
                &account.username,
                Uuid::new_v4(),
                0,
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_encrypt(&folder_key, &folder_key),
                },
            ),
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
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let version = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // create folder to move document to
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AESImpl::generate_key();

        // move document to folder that was never created
        assert_matches!(
            DefaultClient::move_document(
                &account.api_url,
                &account.username,
                doc_id,
                version,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_encrypt(&folder_key, &subfolder_key),
                }
            ),
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
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let version = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // create folder to move document to
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AESImpl::generate_key();

        assert_matches!(
            DefaultClient::create_folder(
                &account.api_url,
                &account.username,
                subfolder_id,
                &random_filename(),
                folder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_encrypt(&folder_key, &subfolder_key),
                },
            ),
            Ok(_)
        );

        // delete document
        assert_matches!(
            DefaultClient::delete_document(
                &account.api_url,
                &account.username,
                doc_id,
                version,
            ),
            Ok(_)
        );

        // move deleted document
        assert_matches!(
            DefaultClient::move_document(
                &account.api_url,
                &account.username,
                doc_id,
                version,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_encrypt(&folder_key, &subfolder_key),
                },
            ),
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
        let (doc, doc_key) = generate_file_metadata(&account, &root, &root_key, FileType::Document);
        let version = DefaultClient::request(
            &account.api_url,
            &account.private_key,
            CreateDocumentRequest::new(
                &account.username,
                &doc,
                aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            ),
        )
        .unwrap()
        .new_metadata_and_content_version;

        // create folder to move document to
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AESImpl::generate_key();

        assert_matches!(
            DefaultClient::create_folder(
                &account.api_url,
                &account.username,
                subfolder_id,
                &random_filename(),
                folder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_encrypt(&folder_key, &subfolder_key),
                },
            ),
            Ok(_)
        );

        // move document
        assert_matches!(
            DefaultClient::move_document(
                &account.api_url,
                &account.username,
                doc_id,
                version - 1,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_encrypt(&folder_key, &subfolder_key),
                },
            ),
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
        let doc_id = Uuid::new_v4();
        let doc_key = AESImpl::generate_key();
        let doc_name = random_filename();
        let version = DefaultClient::create_document(
            &account.api_url,
            &account.username,
            doc_id,
            &doc_name,
            folder_id,
            aes_encrypt(&doc_key, &String::from("doc content").into_bytes()),
            FolderAccessInfo {
                folder_id: folder_id,
                access_key: aes_encrypt(&folder_key, &doc_key),
            },
        )
        .unwrap();

        // create folder to move document to
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AESImpl::generate_key();

        assert_matches!(
            DefaultClient::create_folder(
                &account.api_url,
                &account.username,
                subfolder_id,
                &random_filename(),
                folder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_encrypt(&folder_key, &subfolder_key),
                },
            ),
            Ok(_)
        );

        // create document with same name in that folder
        let doc_id2 = Uuid::new_v4();
        let doc_key2 = AESImpl::generate_key();
        assert_matches!(
            DefaultClient::create_document(
                &account.api_url,
                &account.username,
                doc_id2,
                &doc_name,
                subfolder_id,
                aes_encrypt(&doc_key2, &String::from("doc content").into_bytes()),
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_encrypt(&folder_key, &doc_key2),
                },
            ),
            Ok(_)
        );

        // move document
        assert_matches!(
            DefaultClient::move_document(
                &account.api_url,
                &account.username,
                doc_id,
                version,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_encrypt(&folder_key, &subfolder_key),
                },
            ),
            Err(ApiError::<MoveDocumentError>::Api(
                MoveDocumentError::DocumentPathTaken
            ))
        );
    }
}
