#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use lockbook_crypto::symkey;
    use lockbook_models::file_metadata::FileType;
    use lockbook_models::tree::FileMetaExt;

    use crate::pure_functions::files;
    use crate::service::{file_encryption_service, test_utils};

    #[test]
    fn encrypt_decrypt_metadatum() {
        let account = test_utils::generate_account();
        let key = symkey::generate_key();
        let file = files::create(FileType::Folder, Uuid::new_v4(), "folder", &account.public_key());

        let encrypted_file =
            file_encryption_service::encrypt_metadatum(&account, &key, &file).unwrap();
        let decrypted_file =
            file_encryption_service::decrypt_metadatum(&key, &encrypted_file).unwrap();

        assert_eq!(file, decrypted_file);
    }

    #[test]
    fn encrypt_decrypt_metadata() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Folder, folder.id, "document", &account.public_key());
        let files = [root.clone(), folder.clone(), document.clone()];

        let encrypted_files = file_encryption_service::encrypt_metadata(&account, &files).unwrap();
        let decrypted_files =
            file_encryption_service::decrypt_metadata(&account, &encrypted_files).unwrap();

        assert_eq!(files.find(root.id).unwrap(), decrypted_files.find(root.id).unwrap(),);
        assert_eq!(files.find(folder.id).unwrap(), decrypted_files.find(folder.id).unwrap(),);
        assert_eq!(files.find(document.id).unwrap(), decrypted_files.find(document.id).unwrap(),);
    }
}