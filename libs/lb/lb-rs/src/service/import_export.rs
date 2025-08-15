use crate::Lb;
use crate::model::ValidationFailure;
use crate::model::errors::{LbErr, LbErrKind, LbResult};
use crate::model::file::File;
use crate::model::file_metadata::FileType;
use crate::model::ValidationFailure;
use crate::LbServer;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub enum ImportStatus {
    CalculatedTotal(usize),
    StartingItem(String),
    FinishedItem(File),
}

impl LbServer {
    #[instrument(level = "debug", skip(self, update_status), err(Debug))]
    pub async fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &Option<F>,
    ) -> LbResult<()> {
        if let Some(update_status) = update_status {
            update_status(ImportStatus::CalculatedTotal(get_total_child_count(sources)?));
        }

        let parent = self.get_file_by_id(dest).await?;
        if !parent.is_folder() {
            return Err(LbErrKind::Validation(ValidationFailure::NonFolderWithChildren(dest)))?;
        }

        let import_file_futures = FuturesUnordered::new();

        for source in sources {
            let lb = self.clone();

            import_file_futures.push(async move {
                lb.import_file_recursively(source, dest, update_status)
                    .await
            });
        }

        import_file_futures
            .collect::<Vec<LbResult<()>>>()
            .await
            .into_iter()
            .collect::<LbResult<()>>()
    }

    async fn import_file_recursively<F: Fn(ImportStatus)>(
        &self, disk_path: &Path, dest: Uuid, update_status: &Option<F>,
    ) -> LbResult<()> {
        if let Some(update_status) = update_status {
            update_status(ImportStatus::StartingItem(format!("{}", disk_path.display())));
        }
        if !disk_path.exists() {
            return Err(LbErrKind::DiskPathInvalid.into());
        }

        let name = disk_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(LbErrKind::DiskPathInvalid)?
            .to_string();

        let file_type = if disk_path.is_file() { FileType::Document } else { FileType::Folder };

        let mut tries = 0;
        let mut retry_name = name.clone();
        let file: File;

        loop {
            match self.create_file(&retry_name, &dest, file_type).await {
                Ok(new_file) => {
                    file = new_file;
                    break;
                }
                Err(err)
                    if matches!(
                        err.kind,
                        LbErrKind::Validation(ValidationFailure::PathConflict(_))
                    ) =>
                {
                    tries += 1;
                    retry_name = format!("{name}-{tries}");
                }
                Err(err) => return Err(err),
            }
        }

        match file_type {
            FileType::Document => {
                let content = fs::read(disk_path).map_err(LbErr::from)?;
                self.write_document(file.id, content.as_slice()).await?;
                if let Some(update_status) = update_status {
                    update_status(ImportStatus::FinishedItem(file));
                }
            }
            FileType::Folder => {
                let id = file.id;
                if let Some(update_status) = update_status {
                    update_status(ImportStatus::FinishedItem(file));
                }

                let disk_children = fs::read_dir(disk_path).map_err(LbErr::from)?;

                let import_file_futures = FuturesUnordered::new();

                for disk_child in disk_children {
                    let child_path = disk_child.map_err(LbErr::from)?.path();
                    let lb = self.clone();

                    import_file_futures.push(async move {
                        lb.import_file_recursively(&child_path, id, update_status)
                            .await
                    });
                }

                import_file_futures
                    .collect::<Vec<LbResult<()>>>()
                    .await
                    .into_iter()
                    .collect::<LbResult<()>>()?;
            }

            FileType::Link { .. } => {
                error!("links should not be interpreted!")
            }
        }

        Ok(())
    }

    #[instrument(level = "debug", skip(self, update_status), err(Debug))]
    pub async fn export_file<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, dest: PathBuf, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()> {
        if dest.is_file() {
            return Err(LbErrKind::DiskPathInvalid.into());
        }

        self.export_file_recursively(id, &dest, edit, update_status)
            .await
    }

    pub async fn export_file_recursively<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, disk_path: &Path, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()> {
        let file = self.get_file_by_id(id).await?;

        let new_dest = disk_path.join(&file.name);

        if let Some(update_status) = update_status {
            update_status(ExportFileInfo {
                disk_path: disk_path.to_path_buf(),
                lockbook_path: self.get_path_by_id(file.id).await?,
            });
        }

        match file.file_type {
            FileType::Document => {
                let mut disk_file = if edit {
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(new_dest)
                } else {
                    OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(new_dest)
                }
                .map_err(LbErr::from)?;

                disk_file
                    .write(self.read_document(file.id, true).await?.as_slice())
                    .map_err(LbErr::from)?;
            }
            FileType::Folder => {
                fs::create_dir(new_dest.clone()).map_err(LbErr::from)?;
                let export_file_futures = FuturesUnordered::new();

                for child in self.get_children(&file.id).await? {
                    let lb = self.clone();
                    let new_dest = &new_dest;

                    export_file_futures.push(async move {
                        lb.export_file_recursively(child.id, new_dest, edit, update_status)
                            .await
                    });
                }

                export_file_futures
                    .collect::<Vec<LbResult<()>>>()
                    .await
                    .into_iter()
                    .collect::<LbResult<()>>()?;
            }
            FileType::Link { target } => {
                let export_file_futures = FuturesUnordered::new();
                let lb = self.clone();

                export_file_futures.push(async move {
                    lb.export_file_recursively(target, disk_path, edit, update_status)
                        .await
                });

                export_file_futures
                    .collect::<Vec<LbResult<()>>>()
                    .await
                    .into_iter()
                    .collect::<LbResult<()>>()?;
            }
        }

        Ok(())
    }
}

fn get_total_child_count(paths: &[PathBuf]) -> LbResult<usize> {
    let mut count = 0;
    for p in paths {
        count += get_child_count(p)?;
    }
    Ok(count)
}

fn get_child_count(path: &Path) -> LbResult<usize> {
    let mut count = 1;
    if path.is_dir() {
        let children = fs::read_dir(path).map_err(LbErr::from)?;
        for maybe_child in children {
            let child_path = maybe_child.map_err(LbErr::from)?.path();

            count += get_child_count(&child_path)?;
        }
    }
    Ok(count)
}

#[derive(Debug, Clone)]
pub struct ExportFileInfo {
    pub disk_path: PathBuf,
    pub lockbook_path: String,
}
