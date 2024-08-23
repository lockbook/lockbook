use crate::logic::account::Account;
use crate::logic::file::File;
use crate::logic::file_like::FileLike;
use crate::logic::file_metadata::FileType;
use crate::logic::filename::NameComponents;
use crate::logic::lazy::LazyStaged1;
use crate::logic::signed_file::SignedFile;
use crate::logic::tree_like::TreeLike;
use crate::logic::{symkey, SharedError};
use crate::model::errors::{CoreError, LbError, LbResult};
use crate::repo::docs::AsyncDocs;
use crate::Lb;
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub enum ImportStatus {
    CalculatedTotal(usize),
    StartingItem(String),
    FinishedItem(File),
}

impl Lb {
    pub async fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> LbResult<()> {
        update_status(ImportStatus::CalculatedTotal(get_total_child_count(sources)?));

        let tx = self.ro_tx().await;
        let db = tx.db();

        let tree = db.base_metadata.stage(&db.local_metadata);
        let parent = tree.find(&dest)?;
        if !parent.is_folder() {
            return Err(CoreError::FileNotFolder.into());
        }

        for disk_path in sources {
            // todo this can happen in parallel and be re-written to be more efficient
            self.import_file_recursively(disk_path, &dest, update_status)
                .await?;
        }

        self.cleanup().await?;

        Ok(())
    }

    pub async fn export_file(
        &mut self, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ExportFileInfo)>>,
    ) -> LbResult<()> {
        if destination.is_file() {
            return Err(CoreError::DiskPathInvalid.into());
        }

        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let file = tree.find(&id)?.clone();

        let account = self.get_account()?;

        Self::export_file_recursively(
            &self.docs,
            account,
            &mut tree,
            &file,
            &destination,
            edit,
            &export_progress,
        )
        .await?;

        Ok(())
    }

    async fn export_file_recursively<Base, Local>(
        docs: &AsyncDocs, account: &Account, tree: &mut LazyStaged1<Base, Local>,
        this_file: &Base::F, disk_path: &Path, edit: bool,
        export_progress: &Option<Box<dyn Fn(ExportFileInfo)>>,
    ) -> LbResult<()>
    where
        Base: TreeLike<F = SignedFile>,
        Local: TreeLike<F = Base::F>,
    {
        let dest_with_new = disk_path.join(tree.name_using_links(this_file.id(), account)?);

        if let Some(ref func) = export_progress {
            func(ExportFileInfo {
                disk_path: disk_path.to_path_buf(),
                lockbook_path: tree.id_to_path(this_file.id(), account)?,
            })
        }

        match this_file.file_type() {
            FileType::Folder => {
                let children = tree.children(this_file.id())?;
                fs::create_dir(dest_with_new.clone()).map_err(LbError::from)?;

                for id in children {
                    if !tree.calculate_deleted(&id)? {
                        let file = tree.find(&id)?.clone();
                        Box::pin(Self::export_file_recursively(
                            docs,
                            account,
                            tree,
                            &file,
                            &dest_with_new,
                            edit,
                            export_progress,
                        ))
                        .await?;
                    }
                }
            }
            FileType::Document => {
                let mut file = if edit {
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(dest_with_new)
                } else {
                    OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(dest_with_new)
                }
                .map_err(LbError::from)?;

                let doc = tree.read_document(docs, this_file.id(), account).await?;

                file.write_all(doc.as_slice()).map_err(LbError::from)?;
            }
            FileType::Link { target } => {
                if !tree.calculate_deleted(&target)? {
                    let file = tree.find(&target)?.clone();
                    Box::pin(Self::export_file_recursively(
                        docs,
                        account,
                        tree,
                        &file,
                        disk_path,
                        edit,
                        export_progress,
                    ))
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn import_file_recursively<F: Fn(ImportStatus)>(
        &self, disk_path: &Path, dest: &Uuid, update_status: &F,
    ) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        let account = self.get_account()?;

        update_status(ImportStatus::StartingItem(format!("{}", disk_path.display())));

        let mut disk_paths_with_destinations = vec![(PathBuf::from(disk_path), *dest)];
        loop {
            let (disk_path, dest) = match disk_paths_with_destinations.pop() {
                None => break,
                Some((disk_path, dest)) => (disk_path, dest),
            };

            if !disk_path.exists() {
                return Err(CoreError::DiskPathInvalid.into());
            }

            let disk_file_name = disk_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(CoreError::DiskPathInvalid)?;

            let ftype = match disk_path.is_file() {
                true => FileType::Document,
                false => FileType::Folder,
            };

            let file_name =
                Self::generate_non_conflicting_name(&mut tree, account, &dest, disk_file_name)?;

            let id = tree.create(
                Uuid::new_v4(),
                symkey::generate_key(),
                &dest,
                &file_name,
                ftype,
                account,
            )?;

            let file = tree.decrypt(account, &id, &db.pub_key_lookup)?;

            tree = if ftype == FileType::Document {
                let doc = fs::read(&disk_path).map_err(LbError::from)?;

                let encrypted_document = tree.update_document(&id, &doc, account)?;
                let hmac = tree.find(&id)?.document_hmac();
                self.docs.insert(&id, hmac, &encrypted_document).await?;

                update_status(ImportStatus::FinishedItem(file));
                tree
            } else {
                update_status(ImportStatus::FinishedItem(file));

                let entries = fs::read_dir(disk_path).map_err(LbError::from)?;

                for entry in entries {
                    let child_path = entry.map_err(LbError::from)?.path();
                    disk_paths_with_destinations.push((child_path.clone(), id));
                }

                tree
            };
        }

        Ok(())
    }

    fn generate_non_conflicting_name<Base, Local>(
        tree: &mut LazyStaged1<Base, Local>, account: &Account, parent: &Uuid, proposed_name: &str,
    ) -> LbResult<String>
    where
        Base: TreeLike<F = SignedFile>,
        Local: TreeLike<F = Base::F>,
    {
        let maybe_siblings = tree.children(parent)?;
        let mut new_name = NameComponents::from(proposed_name);

        let mut siblings = HashSet::new();

        for id in maybe_siblings {
            if !tree.calculate_deleted(&id)? {
                siblings.insert(id);
            }
        }

        let siblings: Result<Vec<String>, SharedError> = siblings
            .iter()
            .map(|id| tree.name_using_links(id, account))
            .collect();
        let siblings = siblings?;

        loop {
            if !siblings.iter().any(|name| *name == new_name.to_name()) {
                return Ok(new_name.to_name());
            }
            new_name = new_name.generate_next();
        }
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
        let children = fs::read_dir(path).map_err(LbError::from)?;
        for maybe_child in children {
            let child_path = maybe_child.map_err(LbError::from)?.path();

            count += get_child_count(&child_path)?;
        }
    }
    Ok(count)
}

pub struct ExportFileInfo {
    pub disk_path: PathBuf,
    pub lockbook_path: String,
}
