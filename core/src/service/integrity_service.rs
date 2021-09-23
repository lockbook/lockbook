use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::{file_repo, root_repo};
use crate::service::file_service;
use crate::{utils, CoreError};
use uuid::Uuid;

#[derive(Debug)]
pub enum TestRepoError {
    NoRootFolder,
    DocumentTreatedAsFolder(Uuid),
    FileOrphaned(Uuid),
    CycleDetected(Uuid),
    FileNameEmpty(Uuid),
    FileNameContainsSlash(Uuid),
    NameConflictDetected(Uuid),
    Core(CoreError),
}

pub fn test_repo_integrity(config: &Config) -> Result<(), TestRepoError> {
    let root_id = root_repo::maybe_get(config)
        .map_err(TestRepoError::Core)?
        .ok_or(TestRepoError::NoRootFolder)?;
    let root = file_repo::maybe_get_metadata(config, RepoSource::Local, root_id)
        .map_err(TestRepoError::Core)?
        .ok_or(TestRepoError::NoRootFolder)?;
    let files =
        file_repo::get_all_metadata(config, RepoSource::Local).map_err(TestRepoError::Core)?;

    let maybe_doc_with_children = utils::filter_documents(&files)
        .into_iter()
        .find(|d| !utils::find_children(&files, d.id).is_empty());
    if let Some(doc) = maybe_doc_with_children {
        return Err(TestRepoError::DocumentTreatedAsFolder(doc.id));
    }

    let root_with_descendants =
        utils::find_with_descendants(&files, root.id).map_err(TestRepoError::Core)?;
    let maybe_non_root_descendant = files
        .iter()
        .filter(|f| utils::maybe_find(&root_with_descendants, f.id).is_none())
        .next();
    if let Some(non_root_descendant) = maybe_non_root_descendant {
        return Err(TestRepoError::FileOrphaned(non_root_descendant.id));
    }

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

    let maybe_self_descendant = file_service::get_invalid_cycles(&files, &[])
        .map_err(TestRepoError::Core)?
        .into_iter()
        .next();
    if let Some(self_descendant) = maybe_self_descendant {
        return Err(TestRepoError::CycleDetected(self_descendant));
    }

    let maybe_path_conflict = file_service::get_path_conflicts(&files, &[])
        .map_err(TestRepoError::Core)?
        .into_iter()
        .next();
    if let Some(path_conflict) = maybe_path_conflict {
        return Err(TestRepoError::NameConflictDetected(path_conflict.existing));
    }

    let mut warnings = Vec::new();
    for file in all.clone() {
        if file.file_type == Document {
            let file_content = file_service::read_document(config, file.id).map_err(Core)?;

            if file_content.len() as u64 == 0 {
                warnings.push(Warning::EmptyFile(file.id));
                continue;
            }

            let file_path = get_path_by_id(config, file.id).map_err(Core)?;
            let extension = Path::new(&file_path)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("");

            if UTF8_SUFFIXES.contains(&extension) && String::from_utf8(file_content).is_err() {
                warnings.push(Warning::InvalidUTF8(file.id));
                continue;
            }

            if extension == "draw" && get_drawing(config, file.id).is_err() {
                warnings.push(Warning::UnreadableDrawing(file.id));
            }
        }
    }

    Ok(warnings)
}
