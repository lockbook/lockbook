mod integration_test;

#[cfg(test)]
mod move_document_tests {
    use crate::integration_test::{
        aes_key, aes_str, generate_account, random_filename, rsa_key, sign,
    };
    use lockbook_core::client::{ApiError, Client, ClientImpl};
    use lockbook_core::model::api::*;
    use lockbook_core::model::crypto::*;
    use lockbook_core::service::crypto_service::{AesImpl, SymmetricCryptoService};
    use uuid::Uuid;

    use crate::assert_matches;

    #[test]
    fn move_document() {
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

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AesImpl::generate_key();
        let version = ClientImpl::create_document(
            &account.api_url,
            &account.username,
            &sign(&account),
            doc_id,
            &random_filename(),
            folder_id,
            aes_str(&doc_key, "doc content"),
            FolderAccessInfo {
                folder_id: folder_id,
                access_key: aes_key(&folder_key, &doc_key),
            },
        )
        .unwrap();

        // create folder to move document to
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::create_folder(
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
            ),
            Ok(_)
        );

        // move document
        assert_matches!(
            ClientImpl::move_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                version,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_key(&folder_key, &subfolder_key),
                }
            ),
            Ok(_)
        );
    }

    #[test]
    fn move_document_not_found() {
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

        // move document that wasn't created
        assert_matches!(
            ClientImpl::move_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                Uuid::new_v4(),
                0,
                folder_id,
                FolderAccessInfo {
                    folder_id: folder_id,
                    access_key: aes_key(&folder_key, &folder_key),
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

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AesImpl::generate_key();
        let version = ClientImpl::create_document(
            &account.api_url,
            &account.username,
            &sign(&account),
            doc_id,
            &random_filename(),
            folder_id,
            aes_str(&doc_key, "doc content"),
            FolderAccessInfo {
                folder_id: folder_id,
                access_key: aes_key(&folder_key, &doc_key),
            },
        )
        .unwrap();

        // create folder to move document to
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AesImpl::generate_key();

        // move document to folder that was never created
        assert_matches!(
            ClientImpl::move_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                version,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_key(&folder_key, &subfolder_key),
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

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AesImpl::generate_key();
        let version = ClientImpl::create_document(
            &account.api_url,
            &account.username,
            &sign(&account),
            doc_id,
            &random_filename(),
            folder_id,
            aes_str(&doc_key, "doc content"),
            FolderAccessInfo {
                folder_id: folder_id,
                access_key: aes_key(&folder_key, &doc_key),
            },
        )
        .unwrap();

        // create folder to move document to
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::create_folder(
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
            ),
            Ok(_)
        );

        // delete document
        assert_matches!(
            ClientImpl::delete_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
            ),
            Ok(_)
        );

        // move deleted document
        assert_matches!(
            ClientImpl::move_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                version,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_key(&folder_key, &subfolder_key),
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

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AesImpl::generate_key();
        let version = ClientImpl::create_document(
            &account.api_url,
            &account.username,
            &sign(&account),
            doc_id,
            &random_filename(),
            folder_id,
            aes_str(&doc_key, "doc content"),
            FolderAccessInfo {
                folder_id: folder_id,
                access_key: aes_key(&folder_key, &doc_key),
            },
        )
        .unwrap();

        // create folder to move document to
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::create_folder(
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
            ),
            Ok(_)
        );

        // move document
        assert_matches!(
            ClientImpl::move_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                version - 1,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_key(&folder_key, &subfolder_key),
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

        // create document
        let doc_id = Uuid::new_v4();
        let doc_key = AesImpl::generate_key();
        let doc_name = random_filename();
        let version = ClientImpl::create_document(
            &account.api_url,
            &account.username,
            &sign(&account),
            doc_id,
            &doc_name,
            folder_id,
            aes_str(&doc_key, "doc content"),
            FolderAccessInfo {
                folder_id: folder_id,
                access_key: aes_key(&folder_key, &doc_key),
            },
        )
        .unwrap();

        // create folder to move document to
        let subfolder_id = Uuid::new_v4();
        let subfolder_key = AesImpl::generate_key();

        assert_matches!(
            ClientImpl::create_folder(
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
            ),
            Ok(_)
        );

        // create document with same name in that folder
        let doc_id2 = Uuid::new_v4();
        let doc_key2 = AesImpl::generate_key();
        assert_matches!(
            ClientImpl::create_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id2,
                &doc_name,
                subfolder_id,
                aes_str(&doc_key2, "doc content"),
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_key(&folder_key, &doc_key2),
                },
            ),
            Ok(_)
        );

        // move document
        assert_matches!(
            ClientImpl::move_document(
                &account.api_url,
                &account.username,
                &sign(&account),
                doc_id,
                version,
                subfolder_id,
                FolderAccessInfo {
                    folder_id: subfolder_id,
                    access_key: aes_key(&folder_key, &subfolder_key),
                },
            ),
            Err(ApiError::<MoveDocumentError>::Api(
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
        MoveFolderError::CannotMoveIntoDescendant
    }
}
