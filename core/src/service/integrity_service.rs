use std::path::Path;

use uuid::Uuid;

use lockbook_models::file_metadata::{EncryptedFileMetadata, FileType};
use lockbook_models::tree::{FileMetaExt, TestFileTreeError, TreeError};

use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::pure_functions::drawing;
use crate::repo::{metadata_repo, root_repo};
use crate::service::integrity_service::TestRepoError::DocumentReadError;
use crate::service::{file_service, path_service};
use crate::CoreError;

const UTF8_SUFFIXES: [&str; 12] = [
    "md", "txt", "text", "markdown", "sh", "zsh", "bash", "html", "css", "js", "csv", "rs",
];

#[derive(Debug, Clone)]
pub enum TestRepoError {
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
    root_repo::maybe_get(config)
        .map_err(TestRepoError::Core)?
        .ok_or(TestRepoError::NoRootFolder)?;

    let files_encrypted = &metadata_repo::get_all(config, RepoSource::Base)
        .map_err(TestRepoError::Core)?
        .stage(&metadata_repo::get_all(config, RepoSource::Local).map_err(TestRepoError::Core)?)
        .into_iter()
        .map(|(f, _)| f)
        .collect::<Vec<EncryptedFileMetadata>>();

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
