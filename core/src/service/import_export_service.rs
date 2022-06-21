use crate::model::filename::NameComponents;
use crate::model::repo::RepoSource;
use crate::{Config, CoreError, RequestContext};
use lockbook_models::file_metadata::{DecryptedFileMetadata, DecryptedFiles, FileType};
use lockbook_models::tree::{FileMetaMapExt, FileMetadata};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub enum ImportStatus {
    CalculatedTotal(usize),
    Error(PathBuf, CoreError),
    StartingItem(String),
    FinishedItem(DecryptedFileMetadata),
}

impl RequestContext<'_, '_> {
    pub fn import_files<F: Fn(ImportStatus)>(
        &mut self, config: &Config, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> Result<(), CoreError> {
        let parent = self.get_not_deleted_metadata(RepoSource::Local, dest)?;
        if parent.is_document() {
            return Err(CoreError::FileNotFolder);
        }

        let n_files = get_total_child_count(sources)?;
        update_status(ImportStatus::CalculatedTotal(n_files));

        for disk_path in sources {
            if let Err(err) = self.import_file_recursively(config, disk_path, &dest, update_status)
            {
                update_status(ImportStatus::Error(disk_path.clone(), err));
            }
        }

        Ok(())
    }

    pub fn export_file(
        &mut self, config: &Config, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), CoreError> {
        if destination.is_file() {
            return Err(CoreError::DiskPathInvalid);
        }

        let file_metadata = self.get_not_deleted_metadata(RepoSource::Local, id)?;
        let all = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        self.export_file_recursively(
            config,
            &all,
            &file_metadata,
            &destination,
            edit,
            &export_progress,
        )
    }

    fn import_file_recursively<F: Fn(ImportStatus)>(
        &mut self, config: &Config, disk_path: &Path, dest: &Uuid, update_status: &F,
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

        let file_name = self.generate_non_conflicting_name(dest, disk_file_name)?;
        let file_metadata = self.create_file(config, &file_name, *dest, ftype)?;

        if ftype == FileType::Document {
            let content = fs::read(&disk_path).map_err(CoreError::from)?;
            self.insert_document(config, RepoSource::Local, &file_metadata, &content)?;
            update_status(ImportStatus::FinishedItem(file_metadata));
        } else {
            update_status(ImportStatus::FinishedItem(file_metadata.clone()));

            let entries = fs::read_dir(disk_path).map_err(CoreError::from)?;

            for entry in entries {
                let child_path = entry.map_err(CoreError::from)?.path();
                self.import_file_recursively(
                    config,
                    &child_path,
                    &file_metadata.id,
                    update_status,
                )?;
            }
        }

        Ok(())
    }

    fn generate_non_conflicting_name(
        &mut self, parent: &Uuid, proposed_name: &str,
    ) -> Result<String, CoreError> {
        let sibblings = self.get_children(*parent)?;
        let mut new_name = NameComponents::from(proposed_name);
        loop {
            if !sibblings
                .values()
                .any(|f| f.decrypted_name == new_name.to_name())
            {
                return Ok(new_name.to_name());
            }
            new_name = new_name.generate_next();
        }
    }

    fn export_file_recursively(
        &mut self, config: &Config, all: &DecryptedFiles,
        parent_file_metadata: &DecryptedFileMetadata, disk_path: &Path, edit: bool,
        export_progress: &Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), CoreError> {
        let dest_with_new = disk_path.join(&parent_file_metadata.decrypted_name);

        if let Some(ref func) = export_progress {
            func(ImportExportFileInfo {
                disk_path: disk_path.to_path_buf(),
                lockbook_path: self.get_path_by_id(parent_file_metadata.id)?,
            })
        }

        match parent_file_metadata.file_type {
            FileType::Folder => {
                let children = all.find_children(parent_file_metadata.id);
                fs::create_dir(dest_with_new.clone()).map_err(CoreError::from)?;

                for child in children.values() {
                    self.export_file_recursively(
                        config,
                        all,
                        child,
                        &dest_with_new,
                        edit,
                        export_progress,
                    )?;
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
                    self.read_document(config, RepoSource::Local, parent_file_metadata.id)?
                        .as_slice(),
                )
                .map_err(CoreError::from)?;
            }
            FileType::Link { linked_file: _ } => {
                // todo(sharing): follow links?
            }
        }

        Ok(())
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
