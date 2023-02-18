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
pub use lockbook_shared::api::ServerIndex;
pub use lockbook_shared::api::{
    AccountFilter, AccountIdentifier, AdminSetUserTierInfo, AppStoreAccountState,
    GooglePlayAccountState, PaymentMethod, PaymentPlatform, StripeAccountState, StripeAccountTier,
    SubscriptionInfo, UnixTimeMillis,
};
pub use lockbook_shared::core_config::Config;
pub use lockbook_shared::crypto::DecryptedDocument;
pub use lockbook_shared::drawing::{ColorAlias, ColorRGB, Drawing, Stroke};
pub use lockbook_shared::file::{File, Share, ShareMode};
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

use db_rs::Db;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use itertools::Itertools;
use lockbook_shared::account::Username;
use lockbook_shared::api::{
    AccountInfo, AdminFileInfoResponse, AdminValidateAccount, AdminValidateServer,
};
use lockbook_shared::clock;
use serde_json::{json, value::Value};
use strum::IntoEnumIterator;

use crate::model::errors::Error::UiError;
use crate::repo::CoreDb;
use crate::service::api_service::{Network, Requester};
use crate::service::log_service;
use crate::service::search_service::{SearchResultItem, StartSearchInfo};

pub type Core = CoreLib<Network>;

#[derive(Clone)]
pub struct CoreLib<Client: Requester> {
    inner: Arc<Mutex<CoreState<Client>>>,
}

pub struct CoreState<Client: Requester> {
    pub config: Config,
    pub public_key: Option<PublicKey>,
    pub db: CoreDb,
    pub client: Client,
}

impl Core {
    #[instrument(level = "info", skip_all, err(Debug))]
    pub fn init(config: &Config) -> Result<Self, UnexpectedError> {
        log_service::init(config)?;
        let db =
            CoreDb::init_with_migration(config).map_err(|err| unexpected_only!("{:#?}", err))?;

        let config = config.clone();
        let client = Network::default();

        let state = CoreState { config, public_key: None, db, client };
        let inner = Arc::new(Mutex::new(state));

        Ok(Self { inner })
    }
}

impl<Client: Requester> CoreLib<Client> {
    pub fn in_tx<F, Out>(&self, f: F) -> CoreResult<Out>
    where
        F: FnOnce(&mut CoreState<Client>) -> CoreResult<Out>,
    {
        let mut inner = self.inner.lock()?;
        let tx = inner.db.begin_transaction()?;
        let val = f(&mut inner);
        tx.drop_safely()?;
        val
    }

    #[instrument(level = "info", skip_all, err(Debug))]
    pub fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> Result<Account, Error<CreateAccountError>> {
        Ok(self.in_tx(|s| s.create_account(username, api_url, welcome_doc))?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn import_account(&self, account_string: &str) -> Result<Account, Error<ImportError>> {
        Ok(self.in_tx(|s| s.import_account(account_string))?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn export_account(&self) -> Result<String, Error<AccountExportError>> {
        Ok(self.in_tx(|s| s.export_account())?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn export_account_qr(&self) -> Result<Vec<u8>, Error<AccountExportError>> {
        Ok(self.in_tx(|s| s.export_account_qr())?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_account(&self) -> Result<Account, Error<GetAccountError>> {
        Ok(self.in_tx(|s| s.get_account().cloned())?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_config(&self) -> Result<Config, UnexpectedError> {
        Ok(self.in_tx(|s| Ok(s.config.clone()))?)
    }

    #[instrument(level = "debug", skip(self, name), err(Debug))]
    pub fn create_file(
        &self, name: &str, parent: Uuid, file_type: FileType,
    ) -> Result<File, Error<CreateFileError>> {
        Ok(self.in_tx(|s| s.create_file(name, &parent, file_type))?)
    }

    #[instrument(level = "debug", skip(self, content), err(Debug))]
    pub fn write_document(
        &self, id: Uuid, content: &[u8],
    ) -> Result<(), Error<WriteToDocumentError>> {
        self.in_tx(|s| s.write_document(id, content))?;
        Ok(self.in_tx(|s| s.cleanup())?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_root(&self) -> Result<File, Error<GetRootError>> {
        Ok(self.in_tx(|s| s.root())?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_children(&self, id: Uuid) -> Result<Vec<File>, UnexpectedError> {
        Ok(self.in_tx(|s| s.get_children(&id))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_and_get_children_recursively(
        &self, id: Uuid,
    ) -> Result<Vec<File>, Error<GetAndGetChildrenError>> {
        Ok(self.in_tx(|s| s.get_and_get_children_recursively(&id))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_file_by_id(&self, id: Uuid) -> Result<File, Error<GetFileByIdError>> {
        Ok(self.in_tx(|s| s.get_file_by_id(&id))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn delete_file(&self, id: Uuid) -> Result<(), Error<FileDeleteError>> {
        Ok(self.in_tx(|s| s.delete(&id))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn read_document(&self, id: Uuid) -> Result<DecryptedDocument, Error<ReadDocumentError>> {
        Ok(self.in_tx(|s| s.read_document(id))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn list_metadatas(&self) -> Result<Vec<File>, UnexpectedError> {
        Ok(self.in_tx(|s| s.list_metadatas())?)
    }

    #[instrument(level = "debug", skip(self, new_name), err(Debug))]
    pub fn rename_file(&self, id: Uuid, new_name: &str) -> Result<(), Error<RenameFileError>> {
        Ok(self.in_tx(|s| s.rename_file(&id, new_name))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn move_file(&self, id: Uuid, new_parent: Uuid) -> Result<(), Error<MoveFileError>> {
        Ok(self.in_tx(|s| s.move_file(&id, &new_parent))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn share_file(
        &self, id: Uuid, username: &str, mode: ShareMode,
    ) -> Result<(), Error<ShareFileError>> {
        Ok(self.in_tx(|s| s.share_file(id, username, mode))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_pending_shares(&self) -> Result<Vec<File>, UnexpectedError> {
        Ok(self.in_tx(|s| s.get_pending_shares())?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn delete_pending_share(&self, id: Uuid) -> Result<(), Error<DeletePendingShareError>> {
        Ok(self.in_tx(|s| {
            let pk = s.get_public_key()?;
            s.delete_share(&id, Some(pk))
        })?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn create_link_at_path(
        &self, path_and_name: &str, target_id: Uuid,
    ) -> Result<File, Error<CreateLinkAtPathError>> {
        Ok(self.in_tx(|s| s.create_link_at_path(path_and_name, target_id))?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn create_at_path(
        &self, path_and_name: &str,
    ) -> Result<File, Error<CreateFileAtPathError>> {
        Ok(self.in_tx(|s| s.create_at_path(path_and_name))?)
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_by_path(&self, path: &str) -> Result<File, Error<GetFileByPathError>> {
        Ok(self.in_tx(|s| s.get_by_path(path))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_path_by_id(&self, id: Uuid) -> Result<String, UnexpectedError> {
        Ok(self.in_tx(|s| s.get_path_by_id(id))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn list_paths(&self, filter: Option<Filter>) -> Result<Vec<String>, UnexpectedError> {
        Ok(self.in_tx(|s| s.list_paths(filter))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_local_changes(&self) -> Result<Vec<Uuid>, UnexpectedError> {
        Ok(self.in_tx(|s| {
            Ok(s.db
                .local_metadata
                .data()
                .keys()
                .into_iter()
                .copied()
                .collect_vec())
        })?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn calculate_work(&self) -> Result<WorkCalculated, Error<CalculateWorkError>> {
        Ok(self.in_tx(|s| s.calculate_work())?)
    }

    // todo: expose work calculated (return value)
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn sync(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), Error<SyncAllError>> {
        Ok(self.in_tx(|s| {
            s.sync(f)?;
            s.cleanup()
        })?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_last_synced(&self) -> Result<i64, UnexpectedError> {
        Ok(self.in_tx(|s| Ok(s.db.last_synced.data().copied().unwrap_or(0)))?)
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
    pub fn get_usage(&self) -> Result<UsageMetrics, Error<GetUsageError>> {
        Ok(self.in_tx(|s| s.get_usage())?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_uncompressed_usage(&self) -> Result<UsageItemMetric, Error<GetUsageError>> {
        Ok(self.in_tx(|s| s.get_uncompressed_usage())?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_drawing(&self, id: Uuid) -> Result<Drawing, Error<GetDrawingError>> {
        Ok(self.in_tx(|s| s.get_drawing(id))?)
    }

    #[instrument(level = "debug", skip(self, d), err(Debug))]
    pub fn save_drawing(&self, id: Uuid, d: &Drawing) -> Result<(), Error<SaveDrawingError>> {
        Ok(self.in_tx(|s| {
            s.save_drawing(id, d)?;
            s.cleanup()
        })?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_drawing(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> Result<Vec<u8>, Error<ExportDrawingError>> {
        Ok(self.in_tx(|s| s.export_drawing(id, format, render_theme))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_drawing_to_disk(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>, location: &str,
    ) -> Result<(), Error<ExportDrawingToDiskError>> {
        Ok(self.in_tx(|s| s.export_drawing_to_disk(id, format, render_theme, location))?)
    }

    #[instrument(level = "debug", skip(self, update_status), err(Debug))]
    pub fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> Result<(), Error<ImportFileError>> {
        Ok(self.in_tx(|s| {
            s.import_files(sources, dest, update_status)?;
            s.cleanup()
        })?)
    }

    #[instrument(level = "debug", skip(self, export_progress), err(Debug))]
    pub fn export_file(
        &self, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ImportExportFileInfo)>>,
    ) -> Result<(), Error<ExportFileError>> {
        Ok(self.in_tx(|s| s.export_file(id, destination, edit, export_progress))?)
    }

    #[instrument(level = "debug", skip(self, input), err(Debug))]
    pub fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, UnexpectedError> {
        Ok(self.in_tx(|s| s.search_file_paths(input))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn start_search(&self) -> Result<StartSearchInfo, UnexpectedError> {
        Ok(self.in_tx(|s| s.start_search())?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn validate(&self) -> Result<Vec<Warning>, TestRepoError> {
        self.in_tx(|s| Ok(s.test_repo_integrity()))
            .map_err(TestRepoError::Core)?
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn upgrade_account_stripe(
        &self, account_tier: StripeAccountTier,
    ) -> Result<(), Error<UpgradeAccountStripeError>> {
        Ok(self.in_tx(|s| s.upgrade_account_stripe(account_tier))?)
    }

    #[instrument(level = "debug", skip(self, purchase_token), err(Debug))]
    pub fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> Result<(), Error<UpgradeAccountGooglePlayError>> {
        Ok(self.in_tx(|s| s.upgrade_account_google_play(purchase_token, account_id))?)
    }

    #[instrument(
        level = "debug",
        skip(self, original_transaction_id, app_account_token),
        err(Debug)
    )]
    pub fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> Result<(), Error<UpgradeAccountGooglePlayError>> {
        Ok(self
            .in_tx(|s| s.upgrade_account_app_store(original_transaction_id, app_account_token))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn cancel_subscription(&self) -> Result<(), Error<CancelSubscriptionError>> {
        Ok(self.in_tx(|s| s.cancel_subscription())?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_subscription_info(
        &self,
    ) -> Result<Option<SubscriptionInfo>, Error<GetSubscriptionInfoError>> {
        Ok(self.in_tx(|s| s.get_subscription_info())?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn delete_account(&self) -> Result<(), Error<DeleteAccountError>> {
        Ok(self.in_tx(|s| s.delete_account())?)
    }

    #[instrument(level = "debug", skip(self, username), err(Debug))]
    pub fn admin_disappear_account(
        &self, username: &str,
    ) -> Result<(), Error<AdminDisappearAccount>> {
        Ok(self.in_tx(|s| s.disappear_account(username))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_disappear_file(&self, id: Uuid) -> Result<(), Error<AdminDisappearFileError>> {
        Ok(self.in_tx(|s| s.disappear_file(id))?)
    }

    #[instrument(level = "debug", skip(self, filter), err(Debug))]
    pub fn admin_list_users(
        &self, filter: Option<AccountFilter>,
    ) -> Result<Vec<Username>, Error<AdminListUsersError>> {
        Ok(self.in_tx(|s| s.list_users(filter))?)
    }

    #[instrument(level = "debug", skip(self, identifier), err(Debug))]
    pub fn admin_get_account_info(
        &self, identifier: AccountIdentifier,
    ) -> Result<AccountInfo, Error<AdminGetAccountInfoError>> {
        Ok(self.in_tx(|s| s.get_account_info(identifier))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_validate_account(
        &self, username: &str,
    ) -> Result<AdminValidateAccount, Error<AdminServerValidateError>> {
        Ok(self.in_tx(|s| s.validate_account(username))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_validate_server(
        &self,
    ) -> Result<AdminValidateServer, Error<AdminServerValidateError>> {
        Ok(self.in_tx(|s| s.validate_server())?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_file_info(
        &self, id: Uuid,
    ) -> Result<AdminFileInfoResponse, Error<AdminFileInfoError>> {
        Ok(self.in_tx(|s| s.file_info(id))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_rebuild_index(
        &self, index: ServerIndex,
    ) -> Result<(), Error<AdminRebuildIndexError>> {
        Ok(self.in_tx(|s| s.rebuild_index(index))?)
    }

    #[instrument(level = "debug", skip(self, info), err(Debug))]
    pub fn admin_set_user_tier(
        &self, username: &str, info: AdminSetUserTierInfo,
    ) -> Result<(), Error<AdminSetUserTierError>> {
        Ok(self.in_tx(|s| s.set_user_tier(username, info))?)
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
