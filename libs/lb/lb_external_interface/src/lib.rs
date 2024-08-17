#[macro_use]
extern crate tracing;

pub mod c_interface;
pub mod java_interface;
pub mod json_interface;
pub mod static_state;

mod errors;

use std::path::PathBuf;

use serde_json::{json, value::Value};
use strum::IntoEnumIterator;

pub use lb_rs;
pub use lb_rs::Core;
use lb_rs::{service::search_service::SearchType, *};

use self::errors::*;

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
);

#[derive(Clone)]
pub struct FfiCore {
    core: lb_rs::Core,
}

impl FfiCore {
    pub fn init(config: &Config) -> Result<Self, UnexpectedError> {
        let core = lb_rs::Core::init(config)?;
        Ok(Self { core })
    }

    pub fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> Result<Account, Error<CreateAccountError>> {
        Ok(self.core.create_account(username, api_url, welcome_doc)?)
    }

    pub fn import_account(&self, account_string: &str) -> Result<Account, Error<ImportError>> {
        Ok(self.core.import_account(account_string)?)
    }

    pub fn export_account(&self) -> Result<String, Error<AccountExportError>> {
        Ok(self.core.export_account()?)
    }

    pub fn export_account_qr(&self) -> Result<Vec<u8>, Error<AccountExportError>> {
        Ok(self.core.export_account_qr()?)
    }

    pub fn get_account(&self) -> Result<Account, Error<GetAccountError>> {
        Ok(self.core.get_account()?)
    }

    pub fn create_file(
        &self, name: &str, parent: Uuid, file_type: FileType,
    ) -> Result<File, Error<CreateFileError>> {
        Ok(self.core.create_file(name, parent, file_type)?)
    }

    pub fn write_document(
        &self, id: Uuid, content: &[u8],
    ) -> Result<(), Error<WriteToDocumentError>> {
        Ok(self.core.write_document(id, content)?)
    }

    pub fn get_root(&self) -> Result<File, Error<GetRootError>> {
        Ok(self.core.get_root()?)
    }

    pub fn get_children(&self, id: Uuid) -> Result<Vec<File>, UnexpectedError> {
        self.core.get_children(id)
    }

    pub fn get_and_get_children_recursively(
        &self, id: Uuid,
    ) -> Result<Vec<File>, Error<GetAndGetChildrenError>> {
        Ok(self.core.get_and_get_children_recursively(id)?)
    }

    pub fn get_file_by_id(&self, id: Uuid) -> Result<File, Error<GetFileByIdError>> {
        Ok(self.core.get_file_by_id(id)?)
    }

    pub fn delete_file(&self, id: Uuid) -> Result<(), Error<FileDeleteError>> {
        Ok(self.core.delete_file(id)?)
    }

    pub fn read_document(&self, id: Uuid) -> Result<DecryptedDocument, Error<ReadDocumentError>> {
        Ok(self.core.read_document(id)?)
    }

    pub fn list_metadatas(&self) -> Result<Vec<File>, UnexpectedError> {
        self.core.list_metadatas()
    }

    pub fn rename_file(&self, id: Uuid, new_name: &str) -> Result<(), Error<RenameFileError>> {
        Ok(self.core.rename_file(id, new_name)?)
    }

    pub fn move_file(&self, id: Uuid, new_parent: Uuid) -> Result<(), Error<MoveFileError>> {
        Ok(self.core.move_file(id, new_parent)?)
    }

    pub fn share_file(
        &self, id: Uuid, username: &str, mode: ShareMode,
    ) -> Result<(), Error<ShareFileError>> {
        Ok(self.core.share_file(id, username, mode)?)
    }

    pub fn get_pending_shares(&self) -> Result<Vec<File>, UnexpectedError> {
        self.core.get_pending_shares()
    }

    pub fn delete_pending_share(&self, id: Uuid) -> Result<(), Error<DeletePendingShareError>> {
        Ok(self.core.delete_pending_share(id)?)
    }

    pub fn create_link_at_path(
        &self, path_and_name: &str, target_id: Uuid,
    ) -> Result<File, Error<CreateLinkAtPathError>> {
        Ok(self.core.create_link_at_path(path_and_name, target_id)?)
    }

    pub fn create_at_path(
        &self, path_and_name: &str,
    ) -> Result<File, Error<CreateFileAtPathError>> {
        Ok(self.core.create_at_path(path_and_name)?)
    }

    pub fn get_by_path(&self, path: &str) -> Result<File, Error<GetFileByPathError>> {
        Ok(self.core.get_by_path(path)?)
    }

    pub fn get_path_by_id(&self, id: Uuid) -> Result<String, UnexpectedError> {
        self.core.get_path_by_id(id)
    }

    pub fn list_paths(&self, filter: Option<Filter>) -> Result<Vec<String>, UnexpectedError> {
        self.core.list_paths(filter)
    }

    pub fn get_local_changes(&self) -> Result<Vec<Uuid>, UnexpectedError> {
        self.core.get_local_changes()
    }

    pub fn calculate_work(&self) -> Result<SyncStatus, Error<CalculateWorkError>> {
        Ok(self.core.calculate_work()?)
    }

    pub fn sync(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<(), Error<SyncAllError>> {
        let _ = self.core.sync(f)?;
        Ok(())
    }

    pub fn get_last_synced(&self) -> Result<i64, UnexpectedError> {
        self.core.get_last_synced()
    }

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

    pub fn suggested_docs(&self, settings: RankingWeights) -> Result<Vec<Uuid>, UnexpectedError> {
        self.core.suggested_docs(settings)
    }

    pub fn get_usage(&self) -> Result<UsageMetrics, Error<GetUsageError>> {
        Ok(self.core.get_usage()?)
    }

    pub fn get_uncompressed_usage(&self) -> Result<UsageItemMetric, Error<GetUsageError>> {
        Ok(self.core.get_uncompressed_usage()?)
    }

       pub fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> Result<(), Error<ImportFileError>> {
        Ok(self.core.import_files(sources, dest, update_status)?)
    }

    pub fn export_file(
        &self, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ExportFileInfo)>>,
    ) -> Result<(), Error<ExportFileError>> {
        Ok(self
            .core
            .export_file(id, destination, edit, export_progress)?)
    }

    pub fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, UnexpectedError> {
        self.core.search_file_paths(input)
    }

    pub fn start_search(&self, search_type: SearchType) -> StartSearchInfo {
        self.core.start_search(search_type)
    }

    pub fn upgrade_account_stripe(
        &self, account_tier: StripeAccountTier,
    ) -> Result<(), Error<UpgradeAccountStripeError>> {
        Ok(self.core.upgrade_account_stripe(account_tier)?)
    }

    pub fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> Result<(), Error<UpgradeAccountGooglePlayError>> {
        Ok(self
            .core
            .upgrade_account_google_play(purchase_token, account_id)?)
    }

    pub fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> Result<(), Error<UpgradeAccountGooglePlayError>> {
        Ok(self
            .core
            .upgrade_account_app_store(original_transaction_id, app_account_token)?)
    }

    pub fn cancel_subscription(&self) -> Result<(), Error<CancelSubscriptionError>> {
        Ok(self.core.cancel_subscription()?)
    }

    pub fn get_subscription_info(
        &self,
    ) -> Result<Option<SubscriptionInfo>, Error<GetSubscriptionInfoError>> {
        Ok(self.core.get_subscription_info()?)
    }

    pub fn delete_account(&self) -> Result<(), Error<DeleteAccountError>> {
        Ok(self.core.delete_account()?)
    }

    pub fn debug_info(&self, os_info: String) -> String {
        self.core.debug_info(os_info)
    }
}
