#![recursion_limit = "256"]

#[macro_use]
extern crate tracing;

pub mod external_interface;
pub mod model;
pub mod pure_functions;
pub mod repo;
pub mod service;

pub use uuid::Uuid;

pub use lockbook_models::account::Account;
pub use lockbook_models::api::{PaymentMethod, PaymentPlatform};
pub use lockbook_models::api::{StripeAccountTier, SubscriptionInfo};
pub use lockbook_models::crypto::DecryptedDocument;
pub use lockbook_models::drawing::{ColorAlias, ColorRGB, Drawing};
pub use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
pub use lockbook_models::tree::{FileMetaMapExt, FileMetaVecExt, FileMetadata};
pub use lockbook_models::work_unit::{ClientWorkUnit, WorkUnit};

pub use crate::model::errors::*;
pub use crate::pure_functions::drawing::SupportedImageFormats;
pub use crate::service::import_export_service::{ImportExportFileInfo, ImportStatus};
pub use crate::service::path_service::Filter;
pub use crate::service::sync_service::{SyncProgress, WorkCalculated};
pub use crate::service::usage_service::{bytes_to_human, UsageItemMetric, UsageMetrics};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use basic_human_duration::ChronoHumanDuration;
use chrono::Duration;
use hmdb::log::Reader;
use hmdb::transaction::Transaction;
use libsecp256k1::PublicKey;
use serde::Deserialize;
use serde_json::{json, value::Value};
use strum::IntoEnumIterator;

use lockbook_crypto::clock_service;
use lockbook_models::crypto::AESKey;

use crate::model::errors::Error::UiError;
use crate::model::repo::RepoSource;
use crate::repo::schema::{transaction, CoreV1, OneKey, Tx};
use crate::service::log_service;
use crate::service::search_service::SearchResultItem;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub logs: bool,
    pub writeable_path: String,
}

#[derive(Clone, Debug, Default)]
pub struct DataCache {
    pub key_cache: HashMap<Uuid, AESKey>,
    pub public_key: Option<PublicKey>,
}

#[derive(Clone, Debug)]
pub struct Core {
    // TODO not pub?
    pub config: Config,
    pub data_cache: Arc<Mutex<DataCache>>, // Or Rc<RefCell>>
    pub db: CoreV1,
}

impl Core {
    pub fn context<'a, 'b>(
        &'a self, tx: &'a mut Tx<'b>,
    ) -> Result<RequestContext<'a, 'b>, CoreError> {
        let config = &self.config;
        let data_cache = self.data_cache.lock().map_err(|err| {
            CoreError::Unexpected(format!("Could not get key_cache mutex: {:?}", err))
        })?;
        Ok(RequestContext { config, data_cache, tx })
    }
}

pub struct RequestContext<'a, 'b> {
    pub config: &'a Config,
    pub data_cache: MutexGuard<'a, DataCache>,
    pub tx: &'a mut transaction::CoreV1<'b>,
}

impl Core {
    #[instrument(level = "info", skip_all, err(Debug))]
    pub fn init(config: &Config) -> Result<Self, UnexpectedError> {
        if config.logs {
            log_service::init(&config.writeable_path)?;
        }
        let db =
            CoreV1::init(&config.writeable_path).map_err(|err| unexpected_only!("{:#?}", err))?;
        let data_cache = Arc::new(Mutex::new(DataCache::default()));
        let config = config.clone();

        Ok(Self { config, data_cache, db })
    }

    #[instrument(level = "info", skip_all, err(Debug))]
    pub fn create_account(
        &self, username: &str, api_url: &str,
    ) -> Result<Account, Error<CreateAccountError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.create_account(username, api_url))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn import_account(&self, account_string: &str) -> Result<Account, Error<ImportError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.import_account(account_string))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn export_account(&self) -> Result<String, Error<AccountExportError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.export_account())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_account(&self) -> Result<Account, Error<GetAccountError>> {
        let account = self
            .db
            .transaction(|tx| self.context(tx)?.get_account())??;
        Ok(account)
    }

    #[instrument(level = "debug", skip(self, name), err(Debug))]
    pub fn create_file(
        &self, name: &str, parent: Uuid, file_type: FileType,
    ) -> Result<DecryptedFileMetadata, Error<CreateFileError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .create_file(&self.config, name, parent, file_type)
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, content), err(Debug))]
    pub fn write_document(
        &self, id: Uuid, content: &[u8],
    ) -> Result<(), Error<WriteToDocumentError>> {
        let val: Result<_, CoreError> = self.db.transaction(|tx| {
            let metadata = self
                .context(tx)?
                .get_not_deleted_metadata(RepoSource::Local, id)?;
            self.context(tx)?.insert_document(
                &self.config,
                RepoSource::Local,
                &metadata,
                content,
            )?;
            Ok(())
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_root(&self) -> Result<DecryptedFileMetadata, Error<GetRootError>> {
        let val = self.db.transaction(|tx| self.context(tx)?.root())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_children(&self, id: Uuid) -> Result<Vec<DecryptedFileMetadata>, UnexpectedError> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_children(id))??
            .into_values()
            .collect();
        Ok(val)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_and_get_children_recursively(
        &self, id: Uuid,
    ) -> Result<Vec<DecryptedFileMetadata>, Error<GetAndGetChildrenError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_and_get_children_recursively(id))??
            .into_values()
            .collect();

        Ok(val)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_file_by_id(
        &self, id: Uuid,
    ) -> Result<DecryptedFileMetadata, Error<GetFileByIdError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .get_not_deleted_metadata(RepoSource::Local, id)
        })?;

        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn delete_file(&self, id: Uuid) -> Result<(), Error<FileDeleteError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.delete_file(&self.config, id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn read_document(&self, id: Uuid) -> Result<DecryptedDocument, Error<ReadDocumentError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .read_document(&self.config, RepoSource::Local, id)
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn save_document_to_disk(
        &self, id: Uuid, location: &str,
    ) -> Result<(), Error<SaveDocumentToDiskError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .save_document_to_disk(&self.config, id, location)
        })?;

        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn list_metadatas(&self) -> Result<Vec<DecryptedFileMetadata>, UnexpectedError> {
        let val = self
            .db
            .transaction(|tx| {
                self.context(tx)?
                    .get_all_not_deleted_metadata(RepoSource::Local)
            })??
            .into_values()
            .collect();
        Ok(val)
    }

    #[instrument(level = "debug", skip(self, new_name), err(Debug))]
    pub fn rename_file(&self, id: Uuid, new_name: &str) -> Result<(), Error<RenameFileError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.rename_file(&self.config, id, new_name))?;

        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn move_file(&self, id: Uuid, new_parent: Uuid) -> Result<(), Error<MoveFileError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.move_file(&self.config, id, new_parent))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn create_at_path(
        &self, path_and_name: &str,
    ) -> Result<DecryptedFileMetadata, Error<CreateFileAtPathError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .create_at_path(&self.config, path_and_name)
        })??;

        Ok(val)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_by_path(
        &self, path: &str,
    ) -> Result<DecryptedFileMetadata, Error<GetFileByPathError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_by_path(path))??;

        Ok(val)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_path_by_id(&self, id: Uuid) -> Result<String, UnexpectedError> {
        let val: Result<_, CoreError> = self
            .db
            .transaction(|tx| self.context(tx)?.get_path_by_id(id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn list_paths(&self, filter: Option<Filter>) -> Result<Vec<String>, UnexpectedError> {
        let val: Result<_, CoreError> = self
            .db
            .transaction(|tx| self.context(tx)?.list_paths(filter))?;

        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_local_changes(&self) -> Result<Vec<Uuid>, UnexpectedError> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_local_changes(&self.config))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn calculate_work(&self) -> Result<WorkCalculated, Error<CalculateWorkError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.calculate_work(&self.config))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn sync(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), Error<SyncAllError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.sync(&self.config, f))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_last_synced(&self) -> Result<i64, UnexpectedError> {
        Ok(self.db.last_synced.get(&OneKey {})?.unwrap_or(0))
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_last_synced_human_string(&self) -> Result<String, UnexpectedError> {
        let last_synced = self.db.last_synced.get(&OneKey {})?.unwrap_or(0);

        Ok(if last_synced != 0 {
            Duration::milliseconds(clock_service::get_time().0 - last_synced)
                .format_human()
                .to_string()
        } else {
            "never".to_string()
        })
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_usage(&self) -> Result<UsageMetrics, Error<GetUsageError>> {
        let val = self.db.transaction(|tx| self.context(tx)?.get_usage())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_uncompressed_usage(&self) -> Result<UsageItemMetric, Error<GetUsageError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_uncompressed_usage(&self.config))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_drawing(&self, id: Uuid) -> Result<Drawing, Error<GetDrawingError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_drawing(&self.config, id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, drawing_bytes), err(Debug))]
    pub fn save_drawing(
        &self, id: Uuid, drawing_bytes: &[u8],
    ) -> Result<(), Error<SaveDrawingError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .save_drawing(&self.config, id, drawing_bytes)
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_drawing(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> Result<Vec<u8>, Error<ExportDrawingError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .export_drawing(&self.config, id, format, render_theme)
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_drawing_to_disk(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>, location: &str,
    ) -> Result<(), Error<ExportDrawingToDiskError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?.export_drawing_to_disk(
                &self.config,
                id,
                format,
                render_theme,
                location,
            )
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, update_status), err(Debug))]
    pub fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> Result<(), Error<ImportFileError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .import_files(&self.config, sources, dest, update_status)
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, export_progress), err(Debug))]
    pub fn export_file(
        &self, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), Error<ExportFileError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .export_file(&self.config, id, destination, edit, export_progress)
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, input), err(Debug))]
    pub fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, UnexpectedError> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.search_file_paths(input))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn validate(&self) -> Result<Vec<Warning>, TestRepoError> {
        self.db
            .transaction(|tx| self.context(tx)?.test_repo_integrity(&self.config))
            .map_err(CoreError::from)
            .map_err(TestRepoError::Core)?
    }

    #[instrument(level = "debug", err(Debug))]
    pub fn upgrade_account_stripe(
        &self, account_tier: StripeAccountTier,
    ) -> Result<(), Error<UpgradeAccountStripeError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.upgrade_account_stripe(account_tier))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(purchase_token), err(Debug))]
    pub fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> Result<(), Error<UpgradeAccountGooglePlayError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .upgrade_account_google_play(purchase_token, account_id)
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn cancel_subscription(&self) -> Result<(), Error<CancelSubscriptionError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.cancel_subscription())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_subscription_info(
        &self,
    ) -> Result<Option<SubscriptionInfo>, Error<GetSubscriptionInfoError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_subscription_info())?;
        Ok(val?)
    }
}

pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
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

pub static DEFAULT_API_LOCATION: &str = "https://api.prod.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");
