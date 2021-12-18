use std::path::Path;

use uuid::Uuid;

use lockbook_models::file_metadata::{FileMetadata, FileType};

use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::pure_functions::{drawing, files};
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

    let files_encrypted = files::stage_encrypted(
        &metadata_repo::get_all(config, RepoSource::Base).map_err(TestRepoError::Core)?,
        &metadata_repo::get_all(config, RepoSource::Local).map_err(TestRepoError::Core)?,
    )
    .into_iter()
    .map(|(f, _)| f)
    .collect::<Vec<FileMetadata>>();

    for file_encrypted in &files_encrypted {
        if files::maybe_find_encrypted(&files_encrypted, file_encrypted.parent).is_none() {
            return Err(TestRepoError::FileOrphaned(file_encrypted.id));
        }
    }

    let maybe_self_descendant = files::get_invalid_cycles_encrypted(&files_encrypted, &[])
        .map_err(TestRepoError::Core)?
        .into_iter()
        .next();
    if let Some(self_descendant) = maybe_self_descendant {
        return Err(TestRepoError::CycleDetected(self_descendant));
    }

    let all_files =
        file_service::get_all_metadata(config, RepoSource::Local).map_err(TestRepoError::Core)?;
    let maybe_doc_with_children = files::filter_documents(&all_files)
        .into_iter()
        .find(|d| !files::find_children(&all_files, d.id).is_empty());
    if let Some(doc) = maybe_doc_with_children {
        return Err(TestRepoError::DocumentTreatedAsFolder(doc.id));
    }

    let maybe_file_with_empty_name = all_files.iter().find(|f| f.decrypted_name.is_empty());
    if let Some(file_with_empty_name) = maybe_file_with_empty_name {
        return Err(TestRepoError::FileNameEmpty(file_with_empty_name.id));
    }

    let maybe_file_with_name_with_slash = all_files.iter().find(|f| f.decrypted_name.contains('/'));
    if let Some(file_with_name_with_slash) = maybe_file_with_name_with_slash {
        return Err(TestRepoError::FileNameContainsSlash(
            file_with_name_with_slash.id,
        ));
    }

    let maybe_path_conflict = files::get_path_conflicts(&all_files, &[])
        .map_err(TestRepoError::Core)?
        .into_iter()
        .next();
    if let Some(path_conflict) = maybe_path_conflict {
        return Err(TestRepoError::NameConflictDetected(path_conflict.existing));
    }

    let mut warnings = Vec::new();
    for file in files::filter_not_deleted(&all_files) {
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
