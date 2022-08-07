use crate::model::repo::RepoSource;
use crate::repo::document_repo;
use crate::OneKey;
use crate::{Config, CoreError, RequestContext};
use libsecp256k1::PublicKey;
use lockbook_shared::account::Account;
use lockbook_shared::file::File;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::filename::NameComponents;
use lockbook_shared::lazy::LazyStaged1;
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use lockbook_shared::SharedError;
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub enum ImportStatus {
    CalculatedTotal(usize),
    Error(PathBuf, CoreError),
    StartingItem(String),
    FinishedItem(File),
}

impl RequestContext<'_, '_> {
    pub fn import_files<F: Fn(ImportStatus)>(
        &mut self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> Result<(), CoreError> {
        let n_files = get_total_child_count(sources)?;
        update_status(ImportStatus::CalculatedTotal(n_files));

        let public_key = self.get_public_key()?;

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();

        let parent = tree.find(&dest)?;
        if parent.is_document() {
            return Err(CoreError::FileNotFolder);
        }

        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        for disk_path in sources {
            tree = Self::import_file_recursively(
                account,
                self.config,
                &public_key,
                tree,
                disk_path,
                &dest,
                update_status,
            )?;
        }

        Ok(())
    }

    pub fn export_file(
        &mut self, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), CoreError> {
        if destination.is_file() {
            return Err(CoreError::DiskPathInvalid);
        }

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();

        let parent_file_metadata = tree.find(&id)?.clone();

        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        Self::export_file_recursively(
            self.config,
            account,
            &mut tree,
            &parent_file_metadata,
            &destination,
            edit,
            &export_progress,
        )
    }

    fn export_file_recursively<Base, Local>(
        config: &Config, account: &Account, tree: &mut LazyStaged1<Base, Local>,
        parent_file_metadata: &Base::F, disk_path: &Path, edit: bool,
        export_progress: &Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), CoreError>
    where
        Base: Stagable<F = SignedFile>,
        Local: Stagable<F = Base::F>,
    {
        let dest_with_new = disk_path.join(&tree.name(parent_file_metadata.id(), account)?);

        if let Some(ref func) = export_progress {
            func(ImportExportFileInfo {
                disk_path: disk_path.to_path_buf(),
                lockbook_path: tree.id_to_path(parent_file_metadata.id(), account)?,
            })
        }

        match parent_file_metadata.file_type() {
            FileType::Folder => {
                let children = tree.children(parent_file_metadata.id())?;
                fs::create_dir(dest_with_new.clone()).map_err(CoreError::from)?;

                for id in children {
                    if !tree.calculate_deleted(&id)? {
                        RequestContext::export_file_recursively(
                            config,
                            account,
                            tree,
                            &tree.find(&id)?.clone(),
                            &dest_with_new,
                            edit,
                            export_progress,
                        )?;
                    }
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

                let doc =
                    document_repo::get(config, RepoSource::Local, *parent_file_metadata.id())?;

                file.write_all(
                    tree.decrypt_document(parent_file_metadata.id(), &doc, account)?
                        .as_slice(),
                )
                .map_err(CoreError::from)?;
            }
        }

        Ok(())
    }

    fn import_file_recursively<F: Fn(ImportStatus), Base, Local>(
        account: &Account, config: &Config, public_key: &PublicKey,
        mut tree: LazyStaged1<Base, Local>, disk_path: &Path, dest: &Uuid, update_status: &F,
    ) -> Result<LazyStaged1<Base, Local>, CoreError>
    where
        Base: Stagable<F = SignedFile>,
        Local: Stagable<F = Base::F>,
    {
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

        let file_name = RequestContext::generate_non_conflicting_name(
            &mut tree,
            account,
            dest,
            disk_file_name,
        )?;
        let (mut tree, id) = tree.create(dest, &file_name, ftype, account, public_key)?;
        let file = tree.finalize(&id, account)?;

        let tree = if ftype == FileType::Document {
            let doc = fs::read(&disk_path).map_err(CoreError::from)?;
            let (tree, doc) = tree.update_document(&id, &doc, account)?;
            document_repo::insert(config, RepoSource::Local, id, &doc)?;

            update_status(ImportStatus::FinishedItem(file));
            tree
        } else {
            update_status(ImportStatus::FinishedItem(file));

            let entries = fs::read_dir(disk_path).map_err(CoreError::from)?;

            for entry in entries {
                let child_path = entry.map_err(CoreError::from)?.path();
                tree = RequestContext::import_file_recursively(
                    account,
                    config,
                    public_key,
                    tree,
                    &child_path,
                    &id,
                    update_status,
                )?;
            }

            tree
        };

        Ok(tree)
    }

    fn generate_non_conflicting_name<Base, Local>(
        tree: &mut LazyStaged1<Base, Local>, account: &Account, parent: &Uuid, proposed_name: &str,
    ) -> Result<String, CoreError>
    where
        Base: Stagable<F = SignedFile>,
        Local: Stagable<F = Base::F>,
    {
        let maybe_siblings = tree.children(parent)?;
        let mut new_name = NameComponents::from(proposed_name);

        let mut siblings = HashSet::new();

        for id in maybe_siblings {
            if !tree.calculate_deleted(&id)? {
                siblings.insert(id);
            }
        }

        let siblings: Result<Vec<String>, SharedError> =
            siblings.iter().map(|id| tree.name(id, account)).collect();
        let siblings = siblings?;

        loop {
            if !siblings.iter().any(|name| *name == new_name.to_name()) {
                return Ok(new_name.to_name());
            }
            new_name = new_name.generate_next();
        }
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
