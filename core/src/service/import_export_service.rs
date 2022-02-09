use std::fs;
use std::fs::{DirEntry, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};

use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::service::file_service;
use crate::service::path_service;
use crate::CoreError;
use lockbook_models::tree::FileMetaExt;

pub struct ImportExportFileInfo {
    pub disk_path: PathBuf,
    pub lockbook_path: String,
}

pub fn import_file(
    config: &Config,
    disk_path: PathBuf,
    parent: Uuid,
    import_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
) -> Result<(), CoreError> {
    info!("importing file {:?} to {}", disk_path, parent);
    if file_service::get_not_deleted_metadata(config, RepoSource::Local, parent)?.file_type
        != FileType::Folder
    {
        return Err(CoreError::FileNotFolder);
    }

    import_file_recursively(
        config,
        &disk_path,
        &path_service::get_path_by_id(config, parent)?,
        &import_progress,
    )
}

fn import_file_recursively(
    config: &Config,
    disk_path: &Path,
    lockbook_path: &str,
    import_progress: &Option<Box<dyn Fn(ImportExportFileInfo)>>,
) -> Result<(), CoreError> {
    if !disk_path.exists() {
        return Err(CoreError::DiskPathInvalid);
    }

    let is_document = disk_path.is_file();
    let lockbook_path_with_new = format!(
        "{}{}{}",
        lockbook_path,
        disk_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(CoreError::DiskPathInvalid)?,
        if is_document { "" } else { "/" }
    );

    if let Some(ref func) = import_progress {
        func(ImportExportFileInfo {
            disk_path: disk_path.to_path_buf(),
            lockbook_path: lockbook_path.to_string(),
        })
    }

    if is_document {
        let content = fs::read(&disk_path).map_err(CoreError::from)?;
        let file_metadata = match path_service::create_at_path(config, &lockbook_path_with_new) {
            Ok(file_metadata) => file_metadata,
            Err(CoreError::PathTaken) => {
                path_service::get_by_path(config, &lockbook_path_with_new)?
            }
            Err(err) => return Err(err),
        };

        file_service::insert_document(config, RepoSource::Local, &file_metadata, &content)?;
    } else {
        let children: Vec<Result<DirEntry, std::io::Error>> =
            fs::read_dir(disk_path).map_err(CoreError::from)?.collect();

        if children.is_empty() {
            match path_service::create_at_path(config, &lockbook_path_with_new) {
                Ok(_) | Err(CoreError::PathTaken) => {}
                Err(err) => return Err(err),
            }
        } else {
            for maybe_child in children {
                let child_path = maybe_child.map_err(CoreError::from)?.path();

                import_file_recursively(
                    config,
                    &child_path,
                    &lockbook_path_with_new,
                    import_progress,
                )?;
            }
        }
    }

    Ok(())
}

pub fn export_file(
    config: &Config,
    id: Uuid,
    destination: PathBuf,
    edit: bool,
    export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
) -> Result<(), CoreError> {
    info!("exporting file {} to {:?}", id, destination);
    if destination.is_file() {
        return Err(CoreError::DiskPathInvalid);
    }

    let file_metadata = &file_service::get_not_deleted_metadata(config, RepoSource::Local, id)?;
    let all = file_service::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    export_file_recursively(config, &all, file_metadata, &destination, edit, &export_progress)
}

fn export_file_recursively(
    config: &Config,
    all: &[DecryptedFileMetadata],
    parent_file_metadata: &DecryptedFileMetadata,
    disk_path: &Path,
    edit: bool,
    export_progress: &Option<Box<dyn Fn(ImportExportFileInfo)>>,
) -> Result<(), CoreError> {
    let dest_with_new = disk_path.join(&parent_file_metadata.decrypted_name);

    if let Some(ref func) = export_progress {
        func(ImportExportFileInfo {
            disk_path: disk_path.to_path_buf(),
            lockbook_path: crate::path_service::get_path_by_id(config, parent_file_metadata.id)?,
        })
    }

    match parent_file_metadata.file_type {
        FileType::Folder => {
            let children = all.find_children(parent_file_metadata.id);
            fs::create_dir(dest_with_new.clone()).map_err(CoreError::from)?;

            for child in children.iter() {
                export_file_recursively(config, all, child, &dest_with_new, edit, export_progress)?;
            }
        }
        FileType::Document => {
            let mut file = if edit {
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(dest_with_new)
            } else {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(dest_with_new)
            }
            .map_err(CoreError::from)?;

            file.write_all(
                file_service::get_not_deleted_document(
                    config,
                    RepoSource::Local,
                    all,
                    parent_file_metadata.id,
                )?
                .as_slice(),
            )
            .map_err(CoreError::from)?;
        }
    }

    Ok(())
}
