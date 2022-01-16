use std::path::Path;

use uuid::Uuid;

use lockbook_models::file_metadata::{EncryptedFileMetadata, FileType};
use lockbook_models::tree::{FileMetaExt, TestFileTreeError, TreeError};

use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::pure_functions::drawing;
use crate::repo::{account_repo, last_updated_repo, metadata_repo};
use crate::service::integrity_service::TestRepoError::DocumentReadError;
use crate::service::{file_service, path_service};
use crate::CoreError;

const UTF8_SUFFIXES: [&str; 12] = [
    "md", "txt", "text", "markdown", "sh", "zsh", "bash", "html", "css", "js", "csv", "rs",
];

#[derive(Debug, Clone)]
pub enum TestRepoError {
    NoAccount,
    NoRootFolder,
    DocumentTreatedAsFolder(Uuid),
    FileOrphaned(Uuid),
    CycleDetected(Uuid),
    FileNameEmpty(Uuid),
    FileNameContainsSlash(Uuid),
    NameConflictDetected(Uuid),
    DocumentReadError(Uuid, CoreError),
    Tree(TreeError),
    Core(CoreError),
}

#[derive(Debug, Clone)]
pub enum Warning {
    EmptyFile(Uuid),
    InvalidUTF8(Uuid),
    UnreadableDrawing(Uuid),
}

pub fn test_repo_integrity(config: &Config) -> Result<Vec<Warning>, TestRepoError> {
    if account_repo::maybe_get(config)
        .map_err(TestRepoError::Core)?
        .is_none()
    {
        return Err(TestRepoError::NoAccount);
    }

    let files_encrypted = &metadata_repo::get_all(config, RepoSource::Base)
        .map_err(TestRepoError::Core)?
        .stage(&metadata_repo::get_all(config, RepoSource::Local).map_err(TestRepoError::Core)?)
        .into_iter()
        .map(|(f, _)| f)
        .collect::<Vec<EncryptedFileMetadata>>();

    if let Ok(0) = last_updated_repo::get(config) {
    } else if files_encrypted.maybe_find_root().is_none() {
        return Err(TestRepoError::NoRootFolder);
    }

    files_encrypted
        .verify_integrity()
        .map_err(|err| match err {
            TestFileTreeError::NoRootFolder => TestRepoError::NoRootFolder,
            TestFileTreeError::DocumentTreatedAsFolder(e) => {
                TestRepoError::DocumentTreatedAsFolder(e)
            }
            TestFileTreeError::FileOrphaned(e) => TestRepoError::FileOrphaned(e),
            TestFileTreeError::CycleDetected(e) => TestRepoError::CycleDetected(e),
            TestFileTreeError::NameConflictDetected(e) => TestRepoError::NameConflictDetected(e),
            TestFileTreeError::Tree(e) => TestRepoError::Tree(e),
        })?;

    let files =
        file_service::get_all_metadata(config, RepoSource::Local).map_err(TestRepoError::Core)?;

    let maybe_file_with_empty_name = files.iter().find(|f| f.decrypted_name.is_empty());
    if let Some(file_with_empty_name) = maybe_file_with_empty_name {
        return Err(TestRepoError::FileNameEmpty(file_with_empty_name.id));
    }

    let maybe_file_with_name_with_slash = files.iter().find(|f| f.decrypted_name.contains('/'));
    if let Some(file_with_name_with_slash) = maybe_file_with_name_with_slash {
        return Err(TestRepoError::FileNameContainsSlash(
            file_with_name_with_slash.id,
        ));
    }

    let mut warnings = Vec::new();
    for file in files.filter_not_deleted().map_err(TestRepoError::Tree)? {
        if file.file_type == FileType::Document {
            let file_content = file_service::get_document(config, RepoSource::Local, &file)
                .map_err(|err| DocumentReadError(file.id, err))?;

            if file_content.len() as u64 == 0 {
                warnings.push(Warning::EmptyFile(file.id));
                continue;
            }

            let file_path =
                path_service::get_path_by_id(config, file.id).map_err(TestRepoError::Core)?;
            let extension = Path::new(&file_path)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("");

            if UTF8_SUFFIXES.contains(&extension) && String::from_utf8(file_content).is_err() {
                warnings.push(Warning::InvalidUTF8(file.id));
                continue;
            }

            if extension == "draw"
                && drawing::parse_drawing(
                    &file_service::get_document(config, RepoSource::Local, &file)
                        .map_err(TestRepoError::Core)?,
                )
                .is_err()
            {
                warnings.push(Warning::UnreadableDrawing(file.id));
            }
        }
    }

    Ok(warnings)
}

#[cfg(test)]
mod unit_tests {
    use crate::assert_matches;
    use crate::{
        pure_functions::files,
        service::{
            integrity_service::TestFileTreeError,
            test_utils,
        },
    };
    use lockbook_models::tree::FileMetaExt;
    use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
    use uuid::Uuid;

    #[test]
    fn test_file_tree_integrity_empty() {
        let files: Vec<DecryptedFileMetadata> = vec![];
        let result = files.verify_integrity();

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_file_tree_integrity_nonempty_ok() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, folder.id, "document", &account.username);

        let result = [root, folder, document].verify_integrity();

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_file_tree_integrity_no_root() {
        let account = test_utils::generate_account();
        let mut root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, folder.id, "document", &account.username);
        root.parent = folder.id;

        let result = [root, folder, document].verify_integrity();

        assert_eq!(result, Err(TestFileTreeError::NoRootFolder));
    }

    #[test]
    fn test_file_tree_integrity_orphan() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let mut document =
            files::create(FileType::Document, folder.id, "document", &account.username);
        document.parent = Uuid::new_v4();
        let document_id = document.id;

        let result = [root, folder, document].verify_integrity();

        assert_eq!(result, Err(TestFileTreeError::FileOrphaned(document_id)));
    }

    #[test]
    fn test_file_tree_integrity_1cycle() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let mut folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        folder.parent = folder.id;

        let result = [root, folder].verify_integrity();

        assert_matches!(result, Err(TestFileTreeError::CycleDetected(_)));
    }

    #[test]
    fn test_file_tree_integrity_2cycle() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let mut folder1 = files::create(FileType::Folder, root.id, "folder1", &account.username);
        let mut folder2 = files::create(FileType::Folder, root.id, "folder2", &account.username);
        folder1.parent = folder2.id;
        folder2.parent = folder1.id;

        let result = [root, folder1, folder2].verify_integrity();

        assert_matches!(result, Err(TestFileTreeError::CycleDetected(_)));
    }

    #[test]
    fn test_file_tree_integrity_document_treated_as_folder() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let document1 = files::create(FileType::Document, root.id, "document1", &account.username);
        let document2 = files::create(
            FileType::Document,
            document1.id,
            "document2",
            &account.username,
        );
        let document1_id = document1.id;

        let result = [root, document1, document2].verify_integrity();

        assert_eq!(
            result,
            Err(TestFileTreeError::DocumentTreatedAsFolder(document1_id))
        );
    }

    #[test]
    fn test_file_tree_integrity_path_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "file", &account.username);
        let document = files::create(FileType::Document, root.id, "file", &account.username);

        let result = [root, folder, document].verify_integrity();

        assert_matches!(result, Err(TestFileTreeError::NameConflictDetected(_)));
    }
}
