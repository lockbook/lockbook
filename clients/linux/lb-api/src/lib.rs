use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

pub use uuid::Uuid;

pub use lockbook_models::account::Account;
pub use lockbook_models::crypto::DecryptedDocument;
pub use lockbook_models::file_metadata::DecryptedFileMetadata as FileMetadata;
pub use lockbook_models::file_metadata::FileType;
pub use lockbook_models::work_unit::ClientWorkUnit;

pub use lockbook_core::CoreError;
pub use lockbook_core::Error;
pub use lockbook_core::Error::UiError;
pub use lockbook_core::Error::Unexpected;
pub use lockbook_core::UnexpectedError;

pub use lockbook_core::AccountExportError as ExportAccountError;
pub use lockbook_core::CalculateWorkError;
pub use lockbook_core::CreateAccountError;
pub use lockbook_core::CreateFileError;
pub use lockbook_core::ExportFileError;
pub use lockbook_core::FileDeleteError;
pub use lockbook_core::GetAndGetChildrenError;
pub use lockbook_core::GetFileByIdError;
pub use lockbook_core::GetFileByPathError;
pub use lockbook_core::GetRootError;
pub use lockbook_core::GetUsageError;
pub use lockbook_core::ImportError as ImportAccountError;
pub use lockbook_core::ImportFileError;
pub use lockbook_core::MigrationError;
pub use lockbook_core::MoveFileError;
pub use lockbook_core::ReadDocumentError;
pub use lockbook_core::RenameFileError;
pub use lockbook_core::SyncAllError;
pub use lockbook_core::WriteToDocumentError as WriteDocumentError;

pub use lockbook_core::model::state::Config;

pub use lockbook_core::service::db_state_service::State as DbState;
pub use lockbook_core::service::import_export_service::ImportExportFileInfo;
pub use lockbook_core::service::import_export_service::ImportStatus;
pub use lockbook_core::service::search_service::SearchResultItem;
pub use lockbook_core::service::sync_service::SyncProgress;
pub use lockbook_core::service::sync_service::WorkCalculated;
pub use lockbook_core::service::usage_service::UsageItemMetric;
pub use lockbook_core::service::usage_service::UsageMetrics;

pub use lockbook_core::DEFAULT_API_LOCATION;

use lockbook_core::model::filename::NameComponents;

pub trait Api: Send + Sync {
    fn init_logger(&self, log_path: &Path) -> Result<(), UnexpectedError>;

    fn db_state(&self) -> Result<DbState, UnexpectedError>;
    fn migrate_db(&self) -> Result<(), Error<MigrationError>>;

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

    fn calculate_work(&self) -> Result<WorkCalculated, Error<CalculateWorkError>>;
    fn last_synced(&self) -> Result<i64, UnexpectedError>;
    fn uncompressed_usage(&self) -> Result<UsageItemMetric, Error<GetUsageError>>;
    fn usage(&self) -> Result<UsageMetrics, Error<GetUsageError>>;
    fn sync_all(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), Error<SyncAllError>>;
    fn is_syncing(&self) -> bool;

    fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, UnexpectedError>;
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
                    SyncAllError::NoAccount => "No account found.",
                }
                .to_string(),
            ),
            Error::Unexpected(msg) => Self::Major(msg),
        }
    }
}

pub struct DefaultApi {
    cfg: Config,
    sync_lock: Mutex<u8>,
}

impl Default for DefaultApi {
    fn default() -> Self {
        let writeable_path = std::env::var("LOCKBOOK_PATH")
            .unwrap_or(format!("{}/.lockbook", std::env::var("HOME").unwrap()));
        let cfg = Config { writeable_path };

        let sync_lock = Mutex::new(0);

        Self { cfg, sync_lock }
    }
}

impl Api for DefaultApi {
    fn init_logger(&self, log_path: &Path) -> Result<(), UnexpectedError> {
        lockbook_core::init_logger(log_path)
    }

    fn db_state(&self) -> Result<DbState, UnexpectedError> {
        lockbook_core::get_db_state(&self.cfg)
    }

    fn migrate_db(&self) -> Result<(), Error<MigrationError>> {
        lockbook_core::migrate_db(&self.cfg)
    }

    fn create_account(
        &self, username: &str, api_url: &str,
    ) -> Result<Account, Error<CreateAccountError>> {
        lockbook_core::create_account(&self.cfg, username, api_url)
    }

    fn import_account(&self, account_string: &str) -> Result<Account, Error<ImportAccountError>> {
        lockbook_core::import_account(&self.cfg, account_string)
    }

    fn export_account(&self) -> Result<String, Error<ExportAccountError>> {
        lockbook_core::export_account(&self.cfg)
    }

    fn account(&self) -> Result<Option<Account>, String> {
        match lockbook_core::get_account(&self.cfg) {
            Ok(acct) => Ok(Some(acct)),
            Err(err) => match err {
                Error::UiError(lockbook_core::GetAccountError::NoAccount) => Ok(None),
                Error::Unexpected(msg) => Err(msg),
            },
        }
    }

    fn root(&self) -> Result<FileMetadata, Error<GetRootError>> {
        lockbook_core::get_root(&self.cfg)
    }

    fn list_metadatas(&self) -> Result<Vec<FileMetadata>, UnexpectedError> {
        lockbook_core::list_metadatas(&self.cfg)
    }

    fn file_by_id(&self, id: Uuid) -> Result<FileMetadata, Error<GetFileByIdError>> {
        lockbook_core::get_file_by_id(&self.cfg, id)
    }

    fn file_by_path(&self, path: &str) -> Result<FileMetadata, Error<GetFileByPathError>> {
        lockbook_core::get_file_by_path(&self.cfg, path)
    }

    fn children(&self, id: Uuid) -> Result<Vec<FileMetadata>, UnexpectedError> {
        lockbook_core::get_children(&self.cfg, id)
    }

    fn file_and_all_children(
        &self, id: Uuid,
    ) -> Result<Vec<FileMetadata>, Error<GetAndGetChildrenError>> {
        lockbook_core::get_and_get_children_recursively(&self.cfg, id)
    }

    fn path_by_id(&self, id: Uuid) -> Result<String, UnexpectedError> {
        lockbook_core::get_path_by_id(&self.cfg, id)
    }

    fn create_file(
        &self, name: &str, parent: Uuid, ftype: FileType,
    ) -> Result<FileMetadata, Error<CreateFileError>> {
        lockbook_core::create_file(&self.cfg, name, parent, ftype)
    }

    fn rename_file(&self, id: Uuid, new_name: &str) -> Result<(), Error<RenameFileError>> {
        lockbook_core::rename_file(&self.cfg, id, new_name)
    }

    fn move_file(&self, id: Uuid, dest: Uuid) -> Result<(), Error<MoveFileError>> {
        lockbook_core::move_file(&self.cfg, id, dest)
    }

    fn delete_file(&self, id: Uuid) -> Result<(), Error<FileDeleteError>> {
        lockbook_core::delete_file(&self.cfg, id)
    }

    fn read_document(&self, id: Uuid) -> Result<DecryptedDocument, Error<ReadDocumentError>> {
        lockbook_core::read_document(&self.cfg, id)
    }

    fn write_document(&self, id: Uuid, content: &[u8]) -> Result<(), Error<WriteDocumentError>> {
        lockbook_core::write_document(&self.cfg, id, content)
    }

    fn import_files(
        &self, sources: &[PathBuf], dest: Uuid, update_status: Box<dyn Fn(ImportStatus)>,
    ) -> Result<(), Error<ImportFileError>> {
        lockbook_core::import_files(&self.cfg, sources, dest, &update_status)
    }

    fn export_file(
        &self, id: Uuid, dest: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), Error<ExportFileError>> {
        lockbook_core::export_file(&self.cfg, id, dest, edit, export_progress)
    }

    fn calculate_work(&self) -> Result<WorkCalculated, Error<CalculateWorkError>> {
        lockbook_core::calculate_work(&self.cfg)
    }

    fn last_synced(&self) -> Result<i64, UnexpectedError> {
        lockbook_core::get_last_synced(&self.cfg)
    }

    fn uncompressed_usage(&self) -> Result<UsageItemMetric, Error<GetUsageError>> {
        lockbook_core::get_uncompressed_usage(&self.cfg)
    }

    fn usage(&self) -> Result<UsageMetrics, Error<GetUsageError>> {
        lockbook_core::get_usage(&self.cfg)
    }

    fn sync_all(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), Error<SyncAllError>> {
        let _lock = self.sync_lock.lock().unwrap();
        lockbook_core::sync_all(&self.cfg, f)
    }

    fn is_syncing(&self) -> bool {
        self.sync_lock.try_lock().is_err()
    }

    fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, UnexpectedError> {
        lockbook_core::search_file_paths(&self.cfg, input)
    }
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
