#![recursion_limit = "256"]

extern crate reqwest;
#[macro_use]
extern crate tracing;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use basic_human_duration::ChronoHumanDuration;
use chrono::Duration;
use hmdb::log::Reader;
use hmdb::transaction::Transaction;
use serde::Serialize;
use serde_json::{json, value::Value};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use uuid::Uuid;

use lockbook_crypto::clock_service;
use lockbook_models::account::Account;
use lockbook_models::api::AccountTier;
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::drawing::{ColorAlias, ColorRGB, Drawing};
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
use model::errors::Error::UiError;
pub use model::errors::{CoreError, Error, UnexpectedError};
use service::log_service;

use crate::billing_service::CreditCardLast4Digits;
use crate::model::errors::{
    core_err_unexpected, AccountExportError, CalculateWorkError, CreateAccountError,
    CreateFileAtPathError, CreateFileError, ExportDrawingError, ExportDrawingToDiskError,
    ExportFileError, FileDeleteError, GetAccountError, GetAndGetChildrenError, GetCreditCard,
    GetDrawingError, GetFileByIdError, GetFileByPathError, GetRootError, GetUsageError,
    ImportError, ImportFileError, MigrationError, MoveFileError, ReadDocumentError,
    RenameFileError, SaveDocumentToDiskError, SaveDrawingError, SwitchAccountTierError,
    SyncAllError, WriteToDocumentError,
};
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::path_service::Filter;
use crate::pure_functions::drawing::SupportedImageFormats;
use crate::repo::schema::{CoreV1, OneKey, Tx};
use crate::service::import_export_service::{self, ImportExportFileInfo, ImportStatus};
use crate::service::search_service::SearchResultItem;
use crate::service::sync_service::SyncProgress;
use crate::service::usage_service::{UsageItemMetric, UsageMetrics};
use crate::service::{
    account_service, billing_service, db_state_service, drawing_service, file_service,
    path_service, search_service, sync_service, usage_service,
};
use crate::sync_service::WorkCalculated;

#[derive(Clone, Debug)]
pub struct LbCore {
    pub config: Config,
    pub db: CoreV1,
}

impl LbCore {
    pub fn init(config: &Config) -> Result<Self, UnexpectedError> {
        log_service::init(&config.writeable_path)?;
        let db =
            CoreV1::init(&config.writeable_path).map_err(|err| unexpected_only!("{:#?}", err))?;
        let config = config.clone();

        Ok(Self { config, db })
    }

    pub fn create_account(
        &self, username: &str, api_url: &str,
    ) -> Result<Account, Error<CreateAccountError>> {
        let val = self
            .db
            .transaction(|tx| tx.create_account(username, api_url))?;
        Ok(val?)
    }

    pub fn import_account(&self, account_string: &str) -> Result<Account, Error<ImportError>> {
        let val = self
            .db
            .transaction(|tx| tx.import_account(account_string))?;
        Ok(val?)
    }

    pub fn export_account(&self) -> Result<String, Error<AccountExportError>> {
        let val = self.db.transaction(|tx| tx.export_account())?;
        Ok(val?)
    }

    pub fn get_account(&self) -> Result<Account, Error<GetAccountError>> {
        let account = self.db.transaction(|tx| tx.get_account())??;
        Ok(account)
    }

    pub fn create_file(
        &self, name: &str, parent: Uuid, file_type: FileType,
    ) -> Result<DecryptedFileMetadata, Error<CreateFileError>> {
        let val = self
            .db
            .transaction(|tx| tx.create_file(&self.config, name, parent, file_type))?;
        Ok(val?)
    }

    pub fn write_document(
        &self, id: Uuid, content: &[u8],
    ) -> Result<(), Error<WriteToDocumentError>> {
        let val: Result<_, CoreError> = self.db.transaction(|tx| {
            let metadata = tx.get_not_deleted_metadata(RepoSource::Local, id)?;
            tx.insert_document(&self.config, RepoSource::Local, &metadata, content)?;
            Ok(())
        })?;
        Ok(val?)
    }

    pub fn get_root(&self) -> Result<DecryptedFileMetadata, Error<GetRootError>> {
        let val = self.db.transaction(|tx| tx.root())?;
        Ok(val?)
    }

    pub fn get_children(&self, id: Uuid) -> Result<Vec<DecryptedFileMetadata>, UnexpectedError> {
        let val = self.db.transaction(|tx| tx.get_children(id))?;
        Ok(val?)
    }

    pub fn get_and_get_children_recursively(
        &self, id: Uuid,
    ) -> Result<Vec<DecryptedFileMetadata>, Error<GetAndGetChildrenError>> {
        let val = self
            .db
            .transaction(|tx| tx.get_and_get_children_recursively(id))?;

        Ok(val?)
    }

    pub fn get_file_by_id(
        &self, id: Uuid,
    ) -> Result<DecryptedFileMetadata, Error<GetFileByIdError>> {
        let val = self
            .db
            .transaction(|tx| tx.get_not_deleted_metadata(RepoSource::Local, id))?;

        Ok(val?)
    }

    pub fn delete_file(&self, id: Uuid) -> Result<(), Error<FileDeleteError>> {
        let val = self.db.transaction(|tx| tx.delete_file(&self.config, id))?;
        Ok(val?)
    }

    pub fn read_document(&self, id: Uuid) -> Result<DecryptedDocument, Error<ReadDocumentError>> {
        let val = self
            .db
            .transaction(|tx| tx.read_document(&self.config, id))?;
        Ok(val?)
    }

    pub fn save_document_to_disk(
        &self, id: Uuid, location: &str,
    ) -> Result<(), Error<SaveDocumentToDiskError>> {
        let val = self
            .db
            .transaction(|tx| tx.save_document_to_disk(&self.config, id, location))?;

        Ok(val?)
    }

    pub fn list_metadatas(&self) -> Result<Vec<DecryptedFileMetadata>, UnexpectedError> {
        let val = self
            .db
            .transaction(|tx| tx.get_all_not_deleted_metadata(RepoSource::Local))?;
        Ok(val?)
    }

    pub fn rename_file(&self, id: Uuid, new_name: &str) -> Result<(), Error<RenameFileError>> {
        let val = self
            .db
            .transaction(|tx| tx.rename_file(&self.config, id, new_name))?;

        Ok(val?)
    }

    pub fn move_file(&self, id: Uuid, new_parent: Uuid) -> Result<(), Error<MoveFileError>> {
        let val = self
            .db
            .transaction(|tx| tx.move_file(&self.config, id, new_parent))?;
        Ok(val?)
    }

    pub fn create_at_path(
        &self, path_and_name: &str,
    ) -> Result<DecryptedFileMetadata, Error<CreateFileAtPathError>> {
        let val = self
            .db
            .transaction(|tx| tx.create_at_path(&self.config, path_and_name))??;

        Ok(val)
    }

    pub fn get_by_path(
        &self, path: &str,
    ) -> Result<DecryptedFileMetadata, Error<GetFileByPathError>> {
        let val = self.db.transaction(|tx| tx.get_by_path(path))??;

        Ok(val)
    }

    pub fn get_path_by_id(&self, id: Uuid) -> Result<String, UnexpectedError> {
        let val: Result<_, CoreError> = self.db.transaction(|tx| tx.get_path_by_id(id))?;
        Ok(val?)
    }

    pub fn list_paths(&self, filter: Option<Filter>) -> Result<Vec<String>, UnexpectedError> {
        let val: Result<_, CoreError> = self.db.transaction(|tx| tx.list_paths(filter))?;

        Ok(val?)
    }

    pub fn get_local_changes(&self) -> Result<Vec<Uuid>, UnexpectedError> {
        let val = self
            .db
            .transaction(|tx| tx.get_local_changes(&self.config))?;
        Ok(val?)
    }

    pub fn calculate_work(&self) -> Result<WorkCalculated, Error<CalculateWorkError>> {
        let val = self.db.transaction(|tx| tx.calculate_work(&self.config))?;
        Ok(val?)
    }

    pub fn sync(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), Error<SyncAllError>> {
        let val = self.db.transaction(|tx| tx.sync(&self.config, f))?;
        Ok(val?)
    }

    pub fn get_last_synced(&self) -> Result<i64, UnexpectedError> {
        self.db
            .last_synced
            .get(&OneKey {})?
            .ok_or_else(|| unexpected_only!("No prior sync time found"))
    }

    pub fn get_last_synced_human_string(&self) -> Result<String, UnexpectedError> {
        let last_synced = self
            .db
            .last_synced
            .get(&OneKey {})?
            .ok_or_else(|| unexpected_only!("No prior sync time found"))?;

        Ok(if last_synced != 0 {
            Duration::milliseconds(clock_service::get_time().0 - last_synced)
                .format_human()
                .to_string()
        } else {
            "never".to_string()
        })
    }

    pub fn get_usage(&self) -> Result<UsageMetrics, Error<GetUsageError>> {
        let val = self.db.transaction(|tx| tx.get_usage())?;
        Ok(val?)
    }

    pub fn get_uncompressed_usage(&self) -> Result<UsageItemMetric, Error<GetUsageError>> {
        let val = self
            .db
            .transaction(|tx| tx.get_uncompressed_usage(&self.config))?;
        Ok(val?)
    }

    pub fn get_drawing(&self, id: Uuid) -> Result<Drawing, Error<GetDrawingError>> {
        let val = self.db.transaction(|tx| tx.get_drawing(&self.config, id))?;
        Ok(val?)
    }

    pub fn save_drawing(
        &self, id: Uuid, drawing_bytes: &[u8],
    ) -> Result<(), Error<SaveDrawingError>> {
        let val = self
            .db
            .transaction(|tx| tx.save_drawing(&self.config, id, drawing_bytes))?;
        Ok(val?)
    }

    pub fn export_drawing(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> Result<Vec<u8>, Error<ExportDrawingError>> {
        let val = self
            .db
            .transaction(|tx| tx.export_drawing(&self.config, id, format, render_theme))?;
        Ok(val?)
    }

    pub fn export_drawing_to_disk(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>, location: &str,
    ) -> Result<(), Error<ExportDrawingToDiskError>> {
        let val = self.db.transaction(|tx| {
            tx.export_drawing_to_disk(&self.config, id, format, render_theme, location)
        })?;
        Ok(val?)
    }

    pub fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> Result<(), Error<ImportFileError>> {
        let val = self
            .db
            .transaction(|tx| tx.import_files(&self.config, sources, dest, update_status))?;
        Ok(val?)
    }

    pub fn export_file(
        &self, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), Error<ExportFileError>> {
        let val = self.db.transaction(|tx| {
            tx.export_file(&self.config, id, destination, edit, export_progress)
        })?;
        Ok(val?)
    }

    pub fn switch_account_tier(
        &self, new_account_tier: AccountTier,
    ) -> Result<(), Error<SwitchAccountTierError>> {
        let val = self
            .db
            .transaction(|tx| tx.switch_account_tier(new_account_tier))?;
        Ok(val?)
    }

    pub fn get_credit_card(&self) -> Result<CreditCardLast4Digits, Error<GetCreditCard>> {
        let val = self.db.transaction(|tx| tx.get_credit_card())?;
        Ok(val?)
    }

    pub fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, UnexpectedError> {
        let val = self.db.transaction(|tx| tx.search_file_paths(input))?;
        Ok(val?)
    }
}

// This basically generates a function called `get_all_error_variants`,
// which will produce a big json dict of { "Error": ["Values"] }.
// Clients can consume this and attempt deserializing each array of errors to see
// if they are handling all cases
macro_rules! impl_get_variants {
    ( $( $name:ty,)* ) => {
        fn get_all_error_variants() -> Value {
            json!({
                $(stringify!($name): <$name>::iter().collect::<Vec<_>>(),)*
            })
        }
    };
}

// All new errors must be placed in here!
impl_get_variants!(
    MigrationError,
    CreateAccountError,
    ImportError,
    AccountExportError,
    GetAccountError,
    CreateFileAtPathError,
    WriteToDocumentError,
    CreateFileError,
    GetRootError,
    GetFileByIdError,
    GetFileByPathError,
    FileDeleteError,
    ReadDocumentError,
    RenameFileError,
    MoveFileError,
    SyncAllError,
    CalculateWorkError,
    GetUsageError,
    GetDrawingError,
    SaveDrawingError,
    ExportDrawingError,
    ExportDrawingToDiskError,
    SaveDocumentToDiskError,
);

pub mod external_interface;
pub mod model;
pub mod pure_functions;
pub mod repo;
pub mod service;

pub static DEFAULT_API_LOCATION: &str = "https://api.prod.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");
