#![recursion_limit = "256"]

#[macro_use]
extern crate tracing;

pub mod model;
pub mod repo;
pub mod service;

mod external_interface;

pub use base64;
pub use basic_human_duration::ChronoHumanDuration;
pub use chrono::Duration;
pub use libsecp256k1::PublicKey;
pub use uuid::Uuid;

pub use lockbook_shared::account::Account;
pub use lockbook_shared::api::{
    AccountFilter, AccountIdentifier, AppStoreAccountState, GooglePlayAccountState, PaymentMethod,
    PaymentPlatform, StripeAccountTier, SubscriptionInfo,
};
pub use lockbook_shared::core_config::Config;
pub use lockbook_shared::crypto::DecryptedDocument;
pub use lockbook_shared::drawing::{ColorAlias, ColorRGB, Drawing, Stroke};
pub use lockbook_shared::file::File;
pub use lockbook_shared::file::ShareMode;
pub use lockbook_shared::file_like::FileLike;
pub use lockbook_shared::file_metadata::{FileType, Owner};
pub use lockbook_shared::filename::NameComponents;
pub use lockbook_shared::lazy::LazyTree;
pub use lockbook_shared::path_ops::Filter;
pub use lockbook_shared::server_file::ServerFile;
pub use lockbook_shared::tree_like::TreeLike;
pub use lockbook_shared::tree_like::TreeLikeMut;
pub use lockbook_shared::usage::bytes_to_human;
pub use lockbook_shared::work_unit::{ClientWorkUnit, WorkUnit};

pub use crate::model::drawing::SupportedImageFormats;
pub use crate::model::errors::*;
pub use crate::service::import_export_service::{ImportExportFileInfo, ImportStatus};
pub use crate::service::sync_service::{SyncProgress, WorkCalculated};
pub use crate::service::usage_service::{UsageItemMetric, UsageMetrics};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use hmdb::transaction::Transaction as _;
use itertools::Itertools;
use lockbook_shared::account::Username;
use lockbook_shared::api::{
    AccountInfo, AdminFileInfoResponse, AdminValidateAccount, AdminValidateServer,
};
use lockbook_shared::clock;
use lockbook_shared::crypto::AESKey;
use serde_json::{json, value::Value};
use strum::IntoEnumIterator;

use crate::model::errors::Error::UiError;
use crate::service::api_service::{Network, Requester};
use crate::service::log_service;
use crate::service::search_service::{SearchResultItem, StartSearchInfo};

type CoreDb = repo::schema_v3::CoreV3;
pub type OneKey = repo::schema_v3::OneKey;
type Tx<'a> = repo::schema_v3::Tx<'a>;
type Transaction<'a> = repo::schema_v3::transaction::CoreV3<'a>;

#[derive(Clone, Debug, Default)]
pub struct DataCache {
    pub key_cache: HashMap<Uuid, AESKey>,
    pub public_key: Option<PublicKey>,
}

#[derive(Clone, Debug)]
pub struct CoreLib<Client: Requester> {
    // TODO not pub?
    pub config: Config,
    pub data_cache: Arc<Mutex<DataCache>>, // Or Rc<RefCell>>
    pub db: CoreDb,
    pub client: Client,
}

pub type Core = CoreLib<Network>;

impl<Client: Requester> CoreLib<Client> {
    pub fn context<'a, 'b>(
        &'a self, tx: &'a mut Tx<'b>,
    ) -> CoreResult<RequestContext<'a, 'b, Client>> {
        let config = &self.config;
        let data_cache = self.data_cache.lock().map_err(|err| {
            CoreError::Unexpected(format!("Could not get key_cache mutex: {:?}", err))
        })?;
        let client = &self.client;
        Ok(RequestContext { config, data_cache, tx, client })
    }
}

pub struct RequestContext<'a, 'b, Client: Requester> {
    pub config: &'a Config,
    pub data_cache: MutexGuard<'a, DataCache>,
    pub tx: &'a mut Transaction<'b>,
    pub client: &'a Client,
}

impl Core {
    #[instrument(level = "info", skip_all, err(Debug))]
    pub fn init(config: &Config) -> Result<Self, UnexpectedError> {
        log_service::init(config)?;
        let db = CoreDb::init_with_migration(&config.writeable_path)
            .map_err(|err| unexpected_only!("{:#?}", err))?;
        let data_cache = Arc::new(Mutex::new(DataCache::default()));
        let config = config.clone();
        let client = Network::default();

        Ok(Self { config, data_cache, db, client })
    }
}

impl<Client: Requester> CoreLib<Client> {
    #[instrument(level = "info", skip_all, err(Debug))]
    pub fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> Result<Account, Error<CreateAccountError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .create_account(username, api_url, welcome_doc)
        })?;
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
    pub fn export_account_qr(&self) -> Result<Vec<u8>, Error<AccountExportError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.export_account_qr())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_account(&self) -> Result<Account, Error<GetAccountError>> {
        let account = self
            .db
            .transaction(|tx| self.context(tx)?.get_account().map(|f| f.clone()))??;
        Ok(account)
    }

    #[instrument(level = "debug", skip(self, name), err(Debug))]
    pub fn create_file(
        &self, name: &str, parent: Uuid, file_type: FileType,
    ) -> Result<File, Error<CreateFileError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.create_file(name, &parent, file_type))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, content), err(Debug))]
    pub fn write_document(
        &self, id: Uuid, content: &[u8],
    ) -> Result<(), Error<WriteToDocumentError>> {
        self.db
            .transaction(|tx| self.context(tx)?.write_document(id, content))??;
        self.db.transaction(|tx| self.context(tx)?.cleanup())??;
        Ok(())
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_root(&self) -> Result<File, Error<GetRootError>> {
        let val = self.db.transaction(|tx| self.context(tx)?.root())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_children(&self, id: Uuid) -> Result<Vec<File>, UnexpectedError> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_children(&id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_and_get_children_recursively(
        &self, id: Uuid,
    ) -> Result<Vec<File>, Error<GetAndGetChildrenError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_and_get_children_recursively(&id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_file_by_id(&self, id: Uuid) -> Result<File, Error<GetFileByIdError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_file_by_id(&id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn delete_file(&self, id: Uuid) -> Result<(), Error<FileDeleteError>> {
        let val = self.db.transaction(|tx| self.context(tx)?.delete(&id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn read_document(&self, id: Uuid) -> Result<DecryptedDocument, Error<ReadDocumentError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.read_document(id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn list_metadatas(&self) -> Result<Vec<File>, UnexpectedError> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.list_metadatas())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, new_name), err(Debug))]
    pub fn rename_file(&self, id: Uuid, new_name: &str) -> Result<(), Error<RenameFileError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.rename_file(&id, new_name))?;

        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn move_file(&self, id: Uuid, new_parent: Uuid) -> Result<(), Error<MoveFileError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.move_file(&id, &new_parent))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn share_file(
        &self, id: Uuid, username: &str, mode: ShareMode,
    ) -> Result<(), Error<ShareFileError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.share_file(id, username, mode))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_pending_shares(&self) -> Result<Vec<File>, UnexpectedError> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_pending_shares())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn delete_pending_share(&self, id: Uuid) -> Result<(), Error<DeletePendingShareError>> {
        let val = self.db.transaction(|tx| {
            let mut context = self.context(tx)?;
            let public_key = context.get_public_key()?;
            context.delete_share(&id, Some(public_key))
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn create_link_at_path(
        &self, path_and_name: &str, target_id: Uuid,
    ) -> Result<File, Error<CreateLinkAtPathError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .create_link_at_path(path_and_name, target_id)
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn create_at_path(
        &self, path_and_name: &str,
    ) -> Result<File, Error<CreateFileAtPathError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.create_at_path(path_and_name))?;

        Ok(val?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_by_path(&self, path: &str) -> Result<File, Error<GetFileByPathError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_by_path(path))?;

        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_path_by_id(&self, id: Uuid) -> Result<String, UnexpectedError> {
        let val: CoreResult<_> = self
            .db
            .transaction(|tx| self.context(tx)?.get_path_by_id(id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn list_paths(&self, filter: Option<Filter>) -> Result<Vec<String>, UnexpectedError> {
        let val: CoreResult<_> = self
            .db
            .transaction(|tx| self.context(tx)?.list_paths(filter))?;

        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_local_changes(&self) -> Result<Vec<Uuid>, UnexpectedError> {
        let val = self
            .db
            .transaction(|tx| tx.local_metadata.keys().into_iter().copied().collect_vec())?;
        Ok(val)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn calculate_work(&self) -> Result<WorkCalculated, Error<CalculateWorkError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.calculate_work())?;
        Ok(val?)
    }

    // todo: expose work calculated (return value)
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn sync(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), Error<SyncAllError>> {
        let val = self.db.transaction(|tx| self.context(tx)?.sync(f))?;
        self.db.transaction(|tx| self.context(tx)?.cleanup())??;
        val?;
        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_last_synced(&self) -> Result<i64, UnexpectedError> {
        Ok(self.db.last_synced.get(&OneKey {})?.unwrap_or(0))
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_last_synced_human_string(&self) -> Result<String, UnexpectedError> {
        let last_synced = self.get_last_synced()?;

        Ok(if last_synced != 0 {
            Duration::milliseconds(clock::get_time().0 - last_synced)
                .format_human()
                .to_string()
        } else {
            "never".to_string()
        })
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    // pub fn get_most_active_documents(&self) -> Result<Vec<(Uuid, usize)>, Error<GetUsageError>> {
    //     let val = self
    //         .db
    //         .transaction(|tx| self.context(tx)?.suggested_docs())?;
    //     Ok(val?)
    // }
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_usage(&self) -> Result<UsageMetrics, Error<GetUsageError>> {
        let val = self.db.transaction(|tx| self.context(tx)?.get_usage())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_uncompressed_usage(&self) -> Result<UsageItemMetric, Error<GetUsageError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_uncompressed_usage())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_drawing(&self, id: Uuid) -> Result<Drawing, Error<GetDrawingError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_drawing(id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, d), err(Debug))]
    pub fn save_drawing(&self, id: Uuid, d: &Drawing) -> Result<(), Error<SaveDrawingError>> {
        self.db
            .transaction(|tx| self.context(tx)?.save_drawing(id, d))??;
        self.db.transaction(|tx| self.context(tx)?.cleanup())??;
        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_drawing(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> Result<Vec<u8>, Error<ExportDrawingError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.export_drawing(id, format, render_theme))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_drawing_to_disk(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>, location: &str,
    ) -> Result<(), Error<ExportDrawingToDiskError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .export_drawing_to_disk(id, format, render_theme, location)
        })?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, update_status), err(Debug))]
    pub fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> Result<(), Error<ImportFileError>> {
        self.db
            .transaction(|tx| self.context(tx)?.import_files(sources, dest, update_status))??;
        self.db.transaction(|tx| self.context(tx)?.cleanup())??;
        Ok(())
    }

    #[instrument(level = "debug", skip(self, export_progress), err(Debug))]
    pub fn export_file(
        &self, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), Error<ExportFileError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .export_file(id, destination, edit, export_progress)
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
    pub fn start_search(&self) -> Result<StartSearchInfo, UnexpectedError> {
        let val = self.db.transaction(|tx| self.context(tx)?.start_search())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn validate(&self) -> Result<Vec<Warning>, TestRepoError> {
        self.db
            .transaction(|tx| self.context(tx)?.test_repo_integrity())
            .map_err(CoreError::from)
            .map_err(TestRepoError::Core)?
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn upgrade_account_stripe(
        &self, account_tier: StripeAccountTier,
    ) -> Result<(), Error<UpgradeAccountStripeError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.upgrade_account_stripe(account_tier))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, purchase_token), err(Debug))]
    pub fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> Result<(), Error<UpgradeAccountGooglePlayError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .upgrade_account_google_play(purchase_token, account_id)
        })?;
        Ok(val?)
    }

    #[instrument(
        level = "debug",
        skip(self, original_transaction_id, app_account_token),
        err(Debug)
    )]
    pub fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> Result<(), Error<UpgradeAccountGooglePlayError>> {
        let val = self.db.transaction(|tx| {
            self.context(tx)?
                .upgrade_account_app_store(original_transaction_id, app_account_token)
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

    #[instrument(level = "debug", skip(self, username), err(Debug))]
    pub fn admin_disappear_account(
        &self, username: &str,
    ) -> Result<(), Error<AdminDisappearAccount>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.disappear_account(username))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_disappear_file(&self, id: Uuid) -> Result<(), Error<AdminDisappearFileError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.disappear_file(id))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, filter), err(Debug))]
    pub fn admin_list_users(
        &self, filter: Option<AccountFilter>,
    ) -> Result<Vec<Username>, Error<AdminListUsersError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.list_users(filter))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self, identifier), err(Debug))]
    pub fn admin_get_account_info(
        &self, identifier: AccountIdentifier,
    ) -> Result<AccountInfo, Error<AdminGetAccountInfoError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.get_account_info(identifier))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_validate_account(
        &self, username: &str,
    ) -> Result<AdminValidateAccount, Error<AdminServerValidateError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.validate_account(username))?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_validate_server(
        &self,
    ) -> Result<AdminValidateServer, Error<AdminServerValidateError>> {
        let val = self
            .db
            .transaction(|tx| self.context(tx)?.validate_server())?;
        Ok(val?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_file_info(
        &self, id: Uuid,
    ) -> Result<AdminFileInfoResponse, Error<AdminFileInfoError>> {
        let val = self.db.transaction(|tx| self.context(tx)?.file_info(id))?;
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
