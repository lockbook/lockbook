use crate::{CoreError, CoreState, LbError, LbResult, Requester};
use lockbook_shared::account::Account;
use lockbook_shared::core_config::Config;
use lockbook_shared::file::File;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::filename::NameComponents;
use lockbook_shared::lazy::LazyStaged1;
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::{document_repo, symkey, SharedError};
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

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn import_files<F: Fn(ImportStatus)>(
        &mut self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> LbResult<()> {
        update_status(ImportStatus::CalculatedTotal(get_total_child_count(sources)?));

        let tree = self.db.base_metadata.stage(&self.db.local_metadata);
        let parent = tree.find(&dest)?;
        if !parent.is_folder() {
            return Err(CoreError::FileNotFolder.into());
        }

        for disk_path in sources {
            self.import_file_recursively(disk_path, &dest, update_status)?;
        }

        Ok(())
    }

    pub(crate) fn export_file(
        &mut self, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> LbResult<()> {
        if destination.is_file() {
            return Err(CoreError::DiskPathInvalid.into());
        }

        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();

        let file = tree.find(&id)?.clone();

        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        Self::export_file_recursively(
            &self.config,
            account,
            &mut tree,
            &file,
            &destination,
            edit,
            &export_progress,
        )?;

        Ok(())
    }

    fn export_file_recursively<Base, Local>(
        config: &Config, account: &Account, tree: &mut LazyStaged1<Base, Local>,
        this_file: &Base::F, disk_path: &Path, edit: bool,
        export_progress: &Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> LbResult<()>
    where
        Base: TreeLike<F = SignedFile>,
        Local: TreeLike<F = Base::F>,
    {
        let dest_with_new = disk_path.join(tree.name_using_links(this_file.id(), account)?);

        if let Some(ref func) = export_progress {
            func(ImportExportFileInfo {
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
                        CoreState::<Client>::export_file_recursively(
                            config,
                            account,
                            tree,
                            &file,
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
                .map_err(LbError::from)?;

                let doc = tree.read_document(config, this_file.id(), account)?;

                file.write_all(doc.as_slice()).map_err(LbError::from)?;
            }
            FileType::Link { target } => {
                if !tree.calculate_deleted(&target)? {
                    let file = tree.find(&target)?.clone();
                    CoreState::<Client>::export_file_recursively(
                        config,
                        account,
                        tree,
                        &file,
                        disk_path,
                        edit,
                        export_progress,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn import_file_recursively<F: Fn(ImportStatus)>(
        &mut self, disk_path: &Path, dest: &Uuid, update_status: &F,
    ) -> LbResult<()> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

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

            let file_name = CoreState::<Client>::generate_non_conflicting_name(
                &mut tree,
                account,
                &dest,
                disk_file_name,
            )?;

            let id = tree.create(
                Uuid::new_v4(),
                symkey::generate_key(),
                &dest,
                &file_name,
                ftype,
                account,
            )?;
            let file = tree.finalize(&id, account, &mut self.db.pub_key_lookup)?;

            tree = if ftype == FileType::Document {
                let doc = fs::read(&disk_path).map_err(LbError::from)?;

                let encrypted_document = tree.update_document(&id, &doc, account)?;
                let hmac = tree.find(&id)?.document_hmac();
                document_repo::insert(&self.config, &id, hmac, &encrypted_document)?;

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

pub struct ImportExportFileInfo {
    pub disk_path: PathBuf,
    pub lockbook_path: String,
}
