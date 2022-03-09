use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
use lockbook_models::tree::FileMetaExt;

use crate::model::filename::NameComponents;
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::service::file_service;
use crate::CoreError;

pub enum ImportStatus {
    CalculatedTotal(usize),
    Error(PathBuf, CoreError),
    StartingItem(String),
    FinishedItem(DecryptedFileMetadata),
}

pub fn import_files<F: Fn(ImportStatus)>(
    config: &Config, sources: &[PathBuf], dest: Uuid, update_status: &F,
) -> Result<(), CoreError> {
    info!("importing files {:?} to {}", sources, dest);

    let parent = file_service::get_not_deleted_metadata(config, RepoSource::Local, dest)?;
    if parent.file_type == FileType::Document {
        return Err(CoreError::FileNotFolder);
    }

    let n_files = get_total_child_count(sources)?;
    update_status(ImportStatus::CalculatedTotal(n_files));

    for disk_path in sources {
        if let Err(err) = import_file_recursively(config, disk_path, &dest, update_status) {
            update_status(ImportStatus::Error(disk_path.clone(), err));
        }
    }

    Ok(())
}

fn import_file_recursively<F: Fn(ImportStatus)>(
    config: &Config, disk_path: &Path, dest: &Uuid, update_status: &F,
) -> Result<(), CoreError> {
    update_status(ImportStatus::StartingItem(format!("{}", disk_path.display())));

    if !disk_path.exists() {
        return Err(CoreError::DiskPathInvalid);
    }

    let disk_file_name = disk_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(CoreError::DiskPathInvalid)?;

    let ftype = match disk_path.is_file() {
        true => FileType::Document,
        false => FileType::Folder,
    };

    let file_name = generate_non_conflicting_name(config, dest, disk_file_name)?;
    let file_metadata = file_service::create_file(config, &file_name, *dest, ftype)?;

    if ftype == FileType::Document {
        let content = fs::read(&disk_path).map_err(CoreError::from)?;
        file_service::insert_document(config, RepoSource::Local, &file_metadata, &content)?;
        update_status(ImportStatus::FinishedItem(file_metadata));
    } else {
        update_status(ImportStatus::FinishedItem(file_metadata.clone()));

        let entries = fs::read_dir(disk_path).map_err(CoreError::from)?;

        for entry in entries {
            let child_path = entry.map_err(CoreError::from)?.path();
            import_file_recursively(config, &child_path, &file_metadata.id, update_status)?;
        }
    }

    Ok(())
}

fn generate_non_conflicting_name(
    config: &Config, parent: &Uuid, proposed_name: &str,
) -> Result<String, CoreError> {
    let sibblings = file_service::get_children(config, *parent)?;
    let mut new_name = NameComponents::from(proposed_name);
    loop {
        if !sibblings
            .iter()
            .any(|f| f.decrypted_name == new_name.to_name())
        {
            return Ok(new_name.to_name());
        }
        new_name = new_name.generate_next();
    }
}

fn get_total_child_count(paths: &[PathBuf]) -> Result<usize, CoreError> {
    let mut count = 0;
    for p in paths {
        count += get_child_count(p)?;
    }
    Ok(count)
}

fn get_child_count(path: &Path) -> Result<usize, CoreError> {
    let mut count = 1;
    if path.is_dir() {
        let children = std::fs::read_dir(path).map_err(CoreError::from)?;
        for maybe_child in children {
            let child_path = maybe_child.map_err(CoreError::from)?.path();

            count += get_child_count(&child_path)?;
        }
    }
    Ok(count)
}

pub struct ImportExportFileInfo {
    pub disk_path: PathBuf,
    pub lockbook_path: String,
}

pub fn export_file(
    config: &Config, id: Uuid, destination: PathBuf, edit: bool,
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
    config: &Config, all: &[DecryptedFileMetadata], parent_file_metadata: &DecryptedFileMetadata,
    disk_path: &Path, edit: bool, export_progress: &Option<Box<dyn Fn(ImportExportFileInfo)>>,
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
