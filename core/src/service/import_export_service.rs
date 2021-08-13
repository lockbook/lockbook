use crate::model::client_conversion;
use crate::model::client_conversion::ClientFileMetadata;
use crate::model::state::Config;
use crate::repo::file_metadata_repo;
use crate::service::{file_service, path_service};
use crate::CoreError;
use lockbook_models::file_metadata::FileType;
use std::fs;
use std::fs::{DirEntry, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct ImportExportFileInfo {
    pub disk_path: PathBuf,
    pub lockbook_path: String,
}

pub fn import_file(
    config: &Config,
    parent: Uuid,
    source: PathBuf,
    edit: bool,
    f: Option<Box<dyn Fn(ImportExportFileInfo)>>,
) -> Result<(), CoreError> {
    import_file_recursively(
        config,
        &source,
        path_service::get_path_by_id(config, parent)?.as_str(),
        edit,
        &f,
    )
}

fn import_file_recursively(
    config: &Config,
    disk_path: &Path,
    lockbook_path: &str,
    edit: bool,
    f: &Option<Box<dyn Fn(ImportExportFileInfo)>>,
) -> Result<(), CoreError> {
    let is_file = disk_path.is_file();
    let lockbook_path_with_new = format!(
        "{}{}{}",
        lockbook_path,
        disk_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(CoreError::DiskPathInvalid)?,
        if is_file { "" } else { "/" }
    );

    if let Some(ref func) = f {
        func(ImportExportFileInfo {
            disk_path: disk_path.to_path_buf(),
            lockbook_path: lockbook_path.to_string(),
        })
    }

    if disk_path.is_file() {
        let content = fs::read(&disk_path).map_err(CoreError::from)?;
        let file_metadata =
            match path_service::create_at_path(config, lockbook_path_with_new.as_str()) {
                Ok(file_metadata) => file_metadata,
                Err(err) => {
                    if edit && CoreError::PathTaken == err {
                        path_service::get_by_path(config, lockbook_path_with_new.as_str())?
                    } else {
                        return Err(err);
                    }
                }
            };

        file_service::write_document(config, file_metadata.id, content.as_slice())?;
    } else {
        let children: Vec<Result<DirEntry, std::io::Error>> =
            fs::read_dir(disk_path).map_err(CoreError::from)?.collect();

        if children.is_empty() {
            path_service::create_at_path(config, &lockbook_path_with_new)?;
        } else {
            for maybe_child in children {
                let child_path = maybe_child.map_err(CoreError::from)?.path();

                import_file_recursively(config, &child_path, &lockbook_path_with_new, edit, f)?;
            }
        }
    }

    Ok(())
}

pub fn export_file(
    config: &Config,
    parent: Uuid,
    destination: PathBuf,
    edit: bool,
    f: Option<Box<dyn Fn(ImportExportFileInfo)>>,
) -> Result<(), CoreError> {
    if destination.is_file() {
        return Err(CoreError::DiskPathInvalid);
    }

    let file_metadata = client_conversion::generate_client_file_metadata(
        config,
        &file_metadata_repo::get(config, parent)?,
    )?;
    export_file_recursively(config, &file_metadata, &destination, edit, &f)
}

fn export_file_recursively(
    config: &Config,
    parent_file_metadata: &ClientFileMetadata,
    disk_path: &Path,
    edit: bool,
    f: &Option<Box<dyn Fn(ImportExportFileInfo)>>,
) -> Result<(), CoreError> {
    let dest_with_new = disk_path.join(&parent_file_metadata.name);

    if let Some(ref func) = f {
        func(ImportExportFileInfo {
            disk_path: disk_path.to_path_buf(),
            lockbook_path: crate::path_service::get_path_by_id(config, parent_file_metadata.id)?,
        })
    }

    match parent_file_metadata.file_type {
        FileType::Folder => {
            let children =
                file_metadata_repo::get_children_non_recursively(config, parent_file_metadata.id)?;
            fs::create_dir(dest_with_new.clone()).map_err(CoreError::from)?;

            for child in children.iter() {
                let child_file_metadata =
                    client_conversion::generate_client_file_metadata(config, child)?;

                export_file_recursively(config, &child_file_metadata, &dest_with_new, edit, f)?;
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
                file_service::read_document(config, parent_file_metadata.id)?.as_slice(),
            )
            .map_err(CoreError::from)?;
        }
    }

    Ok(())
}
