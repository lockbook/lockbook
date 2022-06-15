mod search;

pub use uuid::Uuid;

pub use lockbook_models::account::Account;
pub use lockbook_models::api::AccountTier;
pub use lockbook_models::api::PaymentMethod;
pub use lockbook_models::crypto::DecryptedDocument;
pub use lockbook_models::drawing::{ColorAlias, ColorRGB};
pub use lockbook_models::file_metadata::DecryptedFileMetadata as FileMetadata;
pub use lockbook_models::file_metadata::FileType;
pub use lockbook_models::work_unit::ClientWorkUnit;

pub use lockbook_core::Config;
pub use lockbook_core::CoreError;
pub use lockbook_core::Error;
pub use lockbook_core::Error::UiError;
pub use lockbook_core::Error::Unexpected;
pub use lockbook_core::UnexpectedError;
pub use lockbook_core::DEFAULT_API_LOCATION;

pub use lockbook_core::model::errors::AccountExportError as ExportAccountError;
pub use lockbook_core::model::errors::CalculateWorkError;
pub use lockbook_core::model::errors::CreateAccountError;
pub use lockbook_core::model::errors::CreateFileError;
pub use lockbook_core::model::errors::ExportDrawingError;
pub use lockbook_core::model::errors::ExportFileError;
pub use lockbook_core::model::errors::FileDeleteError;
pub use lockbook_core::model::errors::GetAndGetChildrenError;
pub use lockbook_core::model::errors::GetCreditCard;
pub use lockbook_core::model::errors::GetFileByIdError;
pub use lockbook_core::model::errors::GetFileByPathError;
pub use lockbook_core::model::errors::GetRootError;
pub use lockbook_core::model::errors::GetUsageError;
pub use lockbook_core::model::errors::ImportError as ImportAccountError;
pub use lockbook_core::model::errors::ImportFileError;
pub use lockbook_core::model::errors::MoveFileError;
pub use lockbook_core::model::errors::ReadDocumentError;
pub use lockbook_core::model::errors::RenameFileError;
pub use lockbook_core::model::errors::SwitchAccountTierError;
pub use lockbook_core::model::errors::SyncAllError;
pub use lockbook_core::model::errors::WriteToDocumentError as WriteDocumentError;

pub use lockbook_core::pure_functions::drawing::SupportedImageFormats;

pub use lockbook_core::service::billing_service::CreditCardLast4Digits;
pub use lockbook_core::service::import_export_service::ImportExportFileInfo;
pub use lockbook_core::service::import_export_service::ImportStatus;
pub use lockbook_core::service::path_service::Filter;
pub use lockbook_core::service::sync_service::SyncProgress;
pub use lockbook_core::service::sync_service::WorkCalculated;
pub use lockbook_core::service::usage_service::bytes_to_human;
pub use lockbook_core::service::usage_service::UsageItemMetric;
pub use lockbook_core::service::usage_service::UsageMetrics;

pub use search::SearchResultItem;
pub use search::Searcher;

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use lockbook_models::tree::FileMetadata as FileMetadataExt;

use lockbook_core::model::filename::NameComponents;
use lockbook_core::Core;

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

    fn searcher(&self, filter: Option<Filter>) -> Result<Searcher, String>;

    fn get_credit_card(&self) -> Result<Option<CreditCardLast4Digits>, String>;
    fn switch_account_tier(
        &self, new_tier: AccountTier,
    ) -> Result<(), Error<SwitchAccountTierError>>;
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
                Error::UiError(lockbook_core::model::errors::GetAccountError::NoAccount) => {
                    Ok(None)
                }
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

    fn searcher(&self, filter: Option<Filter>) -> Result<Searcher, String> {
        let root_name = self
            .root()
            .map_err(|_| "No root!".to_string())?
            .decrypted_name;
        let files = self.list_metadatas()?;

        let mut paths = Vec::new();
        for f in files {
            if filter == None
                || (filter == Some(Filter::FoldersOnly) && f.is_folder())
                || (filter == Some(Filter::DocumentsOnly) && f.is_document())
            {
                let path = self.path_by_id(f.id)?;
                let path_without_root = path.strip_prefix(&root_name).unwrap_or(&path).to_string();
                paths.push((f.id, path_without_root));
            }
        }

        Ok(Searcher::new(paths))
    }

    fn get_credit_card(&self) -> Result<Option<CreditCardLast4Digits>, String> {
        use GetCreditCard::*;
        match self.core.get_credit_card() {
            Ok(last4) => Ok(Some(last4)),
            Err(err) => match err {
                UiError(err) => match err {
                    NoAccount => Err("No account!".to_string()),
                    CouldNotReachServer => Err("Unable to connect to server.".to_string()),
                    ClientUpdateRequired => {
                        Err("You are using an out-of-date app. Please upgrade!".to_string())
                    }
                    NotAStripeCustomer => Ok(None),
                },
                Unexpected(err) => Err(err),
            },
        }
    }

    fn switch_account_tier(
        &self, new_tier: AccountTier,
    ) -> Result<(), Error<SwitchAccountTierError>> {
        self.core.switch_account_tier(new_tier)
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
