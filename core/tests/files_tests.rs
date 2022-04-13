
#[cfg(test)]
mod unit_tests {
    use lockbook_models::file_metadata::FileType;
    use lockbook_models::tree::{FileMetaExt, PathConflict};

    use crate::pure_functions::files::{self};
    use crate::{service::test_utils, CoreError};

    #[test]
    fn apply_rename() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let document_id = document.id;
        files::apply_rename(&[root, folder, document], document_id, "document2").unwrap();
    }

    #[test]
    fn apply_rename_not_found() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let result = files::apply_rename(&[root, folder], document.id, "document2");
        assert_eq!(result, Err(CoreError::FileNonexistent));
    }

    #[test]
    fn apply_rename_root() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let root_id = root.id;
        let result = files::apply_rename(&[root, folder, document], root_id, "root2");
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn apply_rename_invalid_name() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let document_id = document.id;
        let result = files::apply_rename(&[root, folder, document], document_id, "invalid/name");
        assert_eq!(result, Err(CoreError::FileNameContainsSlash));
    }

    #[test]
    fn apply_rename_path_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document1 =
            files::create(FileType::Document, root.id, "document1", &account.public_key());
        let document2 =
            files::create(FileType::Document, root.id, "document2", &account.public_key());

        let document1_id = document1.id;
        let result =
            files::apply_rename(&[root, folder, document1, document2], document1_id, "document2");
        assert_eq!(result, Err(CoreError::PathTaken));
    }

    #[test]
    fn apply_move() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let folder_id = folder.id;
        let document_id = document.id;
        files::apply_move(&[root, folder, document], document_id, folder_id).unwrap();
    }

    #[test]
    fn apply_move_not_found() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let folder_id = folder.id;
        let document_id = document.id;
        let result = files::apply_move(&[root, folder], document_id, folder_id);
        assert_eq!(result, Err(CoreError::FileNonexistent));
    }

    #[test]
    fn apply_move_parent_not_found() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let folder_id = folder.id;
        let document_id = document.id;
        let result = files::apply_move(&[root, document], document_id, folder_id);
        assert_eq!(result, Err(CoreError::FileParentNonexistent));
    }

    #[test]
    fn apply_move_parent_document() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document1 =
            files::create(FileType::Document, root.id, "document1", &account.public_key());
        let document2 =
            files::create(FileType::Document, root.id, "document2", &account.public_key());

        let document1_id = document1.id;
        let document2_id = document2.id;
        let result = files::apply_move(&[root, document1, document2], document2_id, document1_id);
        assert_eq!(result, Err(CoreError::FileNotFolder));
    }

    #[test]
    fn apply_move_root() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let folder_id = folder.id;
        let root_id = root.id;
        let result = files::apply_move(&[root, folder, document], root_id, folder_id);
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn apply_move_path_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document1 =
            files::create(FileType::Document, root.id, "document", &account.public_key());
        let document2 =
            files::create(FileType::Document, folder.id, "document", &account.public_key());

        let folder_id = folder.id;
        let document1_id = document1.id;
        let result =
            files::apply_move(&[root, folder, document1, document2], document1_id, folder_id);
        assert_eq!(result, Err(CoreError::PathTaken));
    }

    #[test]
    fn apply_move_2cycle() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder1 = files::create(FileType::Folder, root.id, "folder1", &account.public_key());
        let folder2 = files::create(FileType::Folder, folder1.id, "folder2", &account.public_key());

        let folder1_id = folder1.id;
        let folder2_id = folder2.id;
        let result = files::apply_move(&[root, folder1, folder2], folder1_id, folder2_id);
        assert_eq!(result, Err(CoreError::FolderMovedIntoSelf));
    }

    #[test]
    fn apply_move_1cycle() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder1", &account.public_key());

        let folder1_id = folder.id;
        let result = files::apply_move(&[root, folder], folder1_id, folder1_id);
        assert_eq!(result, Err(CoreError::FolderMovedIntoSelf));
    }

    #[test]
    fn apply_delete() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let document_id = document.id;
        files::apply_delete(&[root, folder, document], document_id).unwrap();
    }

    #[test]
    fn apply_delete_root() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let root_id = root.id;
        let result = files::apply_delete(&[root, folder, document], root_id);
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn get_nonconflicting_filename() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        assert_eq!(
            files::suggest_non_conflicting_filename(folder.id, &[root, folder], &[]).unwrap(),
            "folder-1"
        );
    }

    #[test]
    fn get_nonconflicting_filename2() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder1 = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let folder2 = files::create(FileType::Folder, root.id, "folder-1", &account.public_key());
        assert_eq!(
            files::suggest_non_conflicting_filename(folder1.id, &[root, folder1, folder2], &[])
                .unwrap(),
            "folder-2"
        );
    }

    #[test]
    fn get_path_conflicts_no_conflicts() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder1 = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let folder2 = files::create(FileType::Folder, root.id, "folder2", &account.public_key());

        let path_conflicts = &[root, folder1].get_path_conflicts(&[folder2]).unwrap();

        assert_eq!(path_conflicts.len(), 0);
    }

    #[test]
    fn get_path_conflicts_one_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder1 = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let folder2 = files::create(FileType::Folder, root.id, "folder", &account.public_key());

        let path_conflicts = &[root, folder1.clone()]
            .get_path_conflicts(&[folder2.clone()])
            .unwrap();

        assert_eq!(path_conflicts.len(), 1);
        assert_eq!(path_conflicts[0], PathConflict { existing: folder1.id, staged: folder2.id });
    }
}