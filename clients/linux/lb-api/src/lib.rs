pub use lockbook_core::service::search_service::SearchResultItem;
pub use lockbook_core::AccountExportError as ExportAccountError;
pub use lockbook_core::DecryptedFileMetadata as FileMetadata;
pub use lockbook_core::Error::UiError;
pub use lockbook_core::Error::Unexpected;
pub use lockbook_core::ImportError as ImportAccountError;
pub use lockbook_core::WriteToDocumentError as WriteDocumentError;
pub use lockbook_core::*;

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use lockbook_core::model::filename::NameComponents;

pub trait Api: Send + Sync {
    fn create_account(
        &self, username: &str, api_url: &str,
    ) -> Result<Account, Error<CreateAccountError>>;
    fn import_account(&self, account_string: &str) -> Result<Account, Error<ImportAccountError>>;
    fn export_account(&self) -> Result<String, Error<ExportAccountError>>;
    fn account(&self) -> Result<Option<Account>, String>;

    fn root(&self) -> Result<FileMetadata, Error<GetRootError>>;
    fn file_by_id(&self, id: Uuid) -> Result<FileMetadata, Error<GetFileByIdError>>;
    fn file_by_path(&self, path: &str) -> Result<FileMetadata, Error<GetFileByPathError>>;
    fn path_by_id(&self, id: Uuid) -> Result<String, UnexpectedError>;

    fn children(&self, id: Uuid) -> Result<Vec<FileMetadata>, UnexpectedError>;
    fn file_and_all_children(
        &self, id: Uuid,
    ) -> Result<Vec<FileMetadata>, Error<GetAndGetChildrenError>>;
    fn list_metadatas(&self) -> Result<Vec<FileMetadata>, UnexpectedError>;

    fn create_file(
        &self, name: &str, parent: Uuid, ftype: FileType,
    ) -> Result<FileMetadata, Error<CreateFileError>>;
    fn rename_file(&self, id: Uuid, new_name: &str) -> Result<(), Error<RenameFileError>>;
    fn move_file(&self, id: Uuid, dest: Uuid) -> Result<(), Error<MoveFileError>>;
    fn delete_file(&self, id: Uuid) -> Result<(), Error<FileDeleteError>>;

    fn read_document(&self, id: Uuid) -> Result<DecryptedDocument, Error<ReadDocumentError>>;
    fn write_document(&self, id: Uuid, content: &[u8]) -> Result<(), Error<WriteDocumentError>>;

    fn import_files(
        &self, sources: &[PathBuf], dest: Uuid, update_status: Box<dyn Fn(ImportStatus)>,
    ) -> Result<(), Error<ImportFileError>>;
    fn export_file(
        &self, id: Uuid, dest: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), Error<ExportFileError>>;

    fn export_drawing(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> Result<Vec<u8>, Error<ExportDrawingError>>;

    fn calculate_work(&self) -> Result<WorkCalculated, Error<CalculateWorkError>>;
    fn last_synced(&self) -> Result<i64, UnexpectedError>;
    fn uncompressed_usage(&self) -> Result<UsageItemMetric, Error<GetUsageError>>;
    fn usage(&self) -> Result<UsageMetrics, Error<GetUsageError>>;
    fn sync_all(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), Error<SyncAllError>>;
    fn is_syncing(&self) -> bool;

    fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, UnexpectedError>;

    fn get_subscription_info(
        &self,
    ) -> Result<Option<SubscriptionInfo>, Error<GetSubscriptionInfoError>>;
    fn upgrade_account(
        &self, new_tier: StripeAccountTier,
    ) -> Result<(), Error<UpgradeAccountStripeError>>;
}

pub enum SyncProgressReport {
    Update(SyncProgress),
    Done(Result<(), SyncError>),
}

pub enum SyncError {
    Major(String),
    Minor(String),
}

impl From<Error<SyncAllError>> for SyncError {
    fn from(err: Error<SyncAllError>) -> Self {
        match err {
            Error::UiError(err) => Self::Minor(
                match err {
                    SyncAllError::CouldNotReachServer => "Offline.",
                    SyncAllError::ClientUpdateRequired => "Client upgrade required.",
                }
                .to_string(),
            ),
            Error::Unexpected(msg) => Self::Major(msg),
        }
    }
}

pub struct DefaultApi {
    core: Core,
    sync_lock: Mutex<u8>,
}

impl DefaultApi {
    pub fn new() -> Result<Self, String> {
        let writeable_path = format!("{}/linux", data_dir());

        let core = Core::init(&Config { logs: true, writeable_path }).map_err(|e| e.0)?;

        let sync_lock = Mutex::new(0);

        Ok(Self { core, sync_lock })
    }
}

impl Api for DefaultApi {
    fn create_account(
        &self, username: &str, api_url: &str,
    ) -> Result<Account, Error<CreateAccountError>> {
        self.core.create_account(username, api_url)
    }

    fn import_account(&self, account_string: &str) -> Result<Account, Error<ImportAccountError>> {
        self.core.import_account(account_string)
    }

    fn export_account(&self) -> Result<String, Error<ExportAccountError>> {
        self.core.export_account()
    }

    fn account(&self) -> Result<Option<Account>, String> {
        match self.core.get_account() {
            Ok(acct) => Ok(Some(acct)),
            Err(err) => match err {
                Error::UiError(lockbook_core::GetAccountError::NoAccount) => Ok(None),
                Error::Unexpected(msg) => Err(msg),
            },
        }
    }

    fn root(&self) -> Result<FileMetadata, Error<GetRootError>> {
        self.core.get_root()
    }

    fn list_metadatas(&self) -> Result<Vec<FileMetadata>, UnexpectedError> {
        self.core.list_metadatas()
    }

    fn file_by_id(&self, id: Uuid) -> Result<FileMetadata, Error<GetFileByIdError>> {
        self.core.get_file_by_id(id)
    }

    fn file_by_path(&self, path: &str) -> Result<FileMetadata, Error<GetFileByPathError>> {
        self.core.get_by_path(path)
    }

    fn children(&self, id: Uuid) -> Result<Vec<FileMetadata>, UnexpectedError> {
        self.core.get_children(id)
    }

    fn file_and_all_children(
        &self, id: Uuid,
    ) -> Result<Vec<FileMetadata>, Error<GetAndGetChildrenError>> {
        self.core.get_and_get_children_recursively(id)
    }

    fn path_by_id(&self, id: Uuid) -> Result<String, UnexpectedError> {
        self.core.get_path_by_id(id)
    }

    fn create_file(
        &self, name: &str, parent: Uuid, ftype: FileType,
    ) -> Result<FileMetadata, Error<CreateFileError>> {
        self.core.create_file(name, parent, ftype)
    }

    fn rename_file(&self, id: Uuid, new_name: &str) -> Result<(), Error<RenameFileError>> {
        self.core.rename_file(id, new_name)
    }

    fn move_file(&self, id: Uuid, dest: Uuid) -> Result<(), Error<MoveFileError>> {
        self.core.move_file(id, dest)
    }

    fn delete_file(&self, id: Uuid) -> Result<(), Error<FileDeleteError>> {
        self.core.delete_file(id)
    }

    fn read_document(&self, id: Uuid) -> Result<DecryptedDocument, Error<ReadDocumentError>> {
        self.core.read_document(id)
    }

    fn write_document(&self, id: Uuid, content: &[u8]) -> Result<(), Error<WriteDocumentError>> {
        self.core.write_document(id, content)
    }

    fn import_files(
        &self, sources: &[PathBuf], dest: Uuid, update_status: Box<dyn Fn(ImportStatus)>,
    ) -> Result<(), Error<ImportFileError>> {
        self.core.import_files(sources, dest, &update_status)
    }

    fn export_file(
        &self, id: Uuid, dest: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), Error<ExportFileError>> {
        self.core.export_file(id, dest, edit, export_progress)
    }

    fn export_drawing(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> Result<Vec<u8>, Error<ExportDrawingError>> {
        self.core.export_drawing(id, format, render_theme)
    }

    fn calculate_work(&self) -> Result<WorkCalculated, Error<CalculateWorkError>> {
        self.core.calculate_work()
    }

    fn last_synced(&self) -> Result<i64, UnexpectedError> {
        self.core.get_last_synced()
    }

    fn usage(&self) -> Result<UsageMetrics, Error<GetUsageError>> {
        self.core.get_usage()
    }

    fn uncompressed_usage(&self) -> Result<UsageItemMetric, Error<GetUsageError>> {
        self.core.get_uncompressed_usage()
    }

    fn sync_all(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), Error<SyncAllError>> {
        let _lock = self.sync_lock.lock().unwrap();
        self.core.sync(f)
    }

    fn is_syncing(&self) -> bool {
        self.sync_lock.try_lock().is_err()
    }

    fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, UnexpectedError> {
        self.core.search_file_paths(input)
    }

    fn get_subscription_info(
        &self,
    ) -> Result<Option<SubscriptionInfo>, Error<GetSubscriptionInfoError>> {
        self.core.get_subscription_info()
    }

    fn upgrade_account(
        &self, new_tier: StripeAccountTier,
    ) -> Result<(), Error<UpgradeAccountStripeError>> {
        self.core.upgrade_account_stripe(new_tier)
    }
}

pub fn data_dir() -> String {
    const ERR_MSG: &str = "Unable to determine a Lockbook data directory.\
 Please consider setting the LOCKBOOK_PATH environment variable.";

    env::var("LOCKBOOK_PATH").unwrap_or_else(|_| {
        format!(
            "{}/.lockbook",
            env::var("HOME").unwrap_or_else(|_| env::var("HOMEPATH").expect(ERR_MSG))
        )
    })
}

pub fn parent_info(api: &Arc<dyn Api>, maybe_id: Option<Uuid>) -> Result<(Uuid, String), String> {
    let id = match maybe_id {
        Some(id) => {
            let meta = api.file_by_id(id).map_err(|e| format!("{:?}", e))?;

            match meta.file_type {
                FileType::Document => meta.parent,
                FileType::Folder => meta.id,
            }
        }
        None => api.root().map_err(|e| format!("{:?}", e))?.id,
    };

    let path = api.path_by_id(id).map_err(|e| format!("{:?}", e))?;

    Ok((id, format!("/{}", path)))
}

pub fn get_non_conflicting_name(siblings: &[FileMetadata], proposed_name: &str) -> String {
    let mut new_name = NameComponents::from(proposed_name);
    loop {
        if !siblings
            .iter()
            .any(|f| f.decrypted_name == new_name.to_name())
        {
            return new_name.to_name();
        }
        new_name = new_name.generate_next();
    }
}
