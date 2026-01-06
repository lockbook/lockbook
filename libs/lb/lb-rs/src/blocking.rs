use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::runtime::Runtime;
use tokio::sync::broadcast::Receiver;
use uuid::Uuid;

use crate::model::account::{Account, Username};
use crate::model::api::{
    AccountFilter, AccountIdentifier, AccountInfo, AdminFileInfoResponse, AdminSetUserTierInfo,
    AdminValidateAccount, AdminValidateServer, ServerIndex, StripeAccountTier, SubscriptionInfo,
};
use crate::model::core_config::Config;
use crate::model::crypto::DecryptedDocument;
use crate::model::errors::{LbResult, Warning};
use crate::model::file::{File, ShareMode};
use crate::model::file_metadata::{DocumentHmac, FileType};
use crate::model::path_ops::Filter;
use crate::service::activity::RankingWeights;
use crate::service::events::Event;
use crate::service::import_export::{ExportFileInfo, ImportStatus};
use crate::service::sync::{SyncProgress, SyncStatus};
use crate::service::usage::{UsageItemMetric, UsageMetrics};
use crate::subscribers::search::{SearchConfig, SearchResult};
use crate::subscribers::status::Status;

#[derive(Clone)]
pub struct Lb {
    lb: crate::Lb,
    rt: Arc<Runtime>,
}

impl Lb {
    pub fn init(config: Config) -> LbResult<Self> {
        let rt = Arc::new(Runtime::new().unwrap());
        let lb = rt.block_on(crate::Lb::init(config))?;
        Ok(Self { rt, lb })
    }

    pub fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        self.rt
            .block_on(self.lb.create_account(username, api_url, welcome_doc))
    }

    pub fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        self.rt.block_on(self.lb.import_account(key, api_url))
    }

    pub fn export_account_private_key(&self) -> LbResult<String> {
        self.lb.export_account_private_key_v1()
    }

    pub fn export_account_phrase(&self) -> LbResult<String> {
        self.lb.export_account_phrase()
    }

    pub fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        self.lb.export_account_qr()
    }

    pub fn get_account(&self) -> LbResult<&Account> {
        self.lb.get_account()
    }

    pub fn get_config(&self) -> Config {
        self.lb.config.clone()
    }

    pub fn create_file(&self, name: &str, parent: &Uuid, file_type: FileType) -> LbResult<File> {
        self.rt
            .block_on(self.lb.create_file(name, parent, file_type))
    }

    pub fn safe_write(
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        self.rt.block_on(self.lb.safe_write(id, old_hmac, content))
    }

    pub fn write_document(&self, id: Uuid, content: &[u8]) -> LbResult<()> {
        self.rt.block_on(self.lb.write_document(id, content))
    }

    pub fn get_root(&self) -> LbResult<File> {
        self.rt.block_on(self.lb.root())
    }

    pub fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        self.rt.block_on(self.lb.get_children(id))
    }

    pub fn get_and_get_children_recursively(&self, id: &Uuid) -> LbResult<Vec<File>> {
        self.rt
            .block_on(self.lb.get_and_get_children_recursively(id))
    }

    pub fn get_file_by_id(&self, id: Uuid) -> LbResult<File> {
        self.rt.block_on(self.lb.get_file_by_id(id))
    }

    pub fn delete_file(&self, id: &Uuid) -> LbResult<()> {
        self.rt.block_on(self.lb.delete(id))
    }

    pub fn read_document(&self, id: Uuid, user_activity: bool) -> LbResult<DecryptedDocument> {
        self.rt.block_on(self.lb.read_document(id, user_activity))
    }

    pub fn read_document_with_hmac(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        self.rt
            .block_on(self.lb.read_document_with_hmac(id, user_activity))
    }

    pub fn list_metadatas(&self) -> LbResult<Vec<File>> {
        self.rt.block_on(self.lb.list_metadatas())
    }

    pub fn rename_file(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        self.rt.block_on(self.lb.rename_file(id, new_name))
    }

    pub fn move_file(&self, id: &Uuid, new_parent: &Uuid) -> LbResult<()> {
        self.rt.block_on(self.lb.move_file(id, new_parent))
    }

    pub fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()> {
        self.rt.block_on(self.lb.share_file(id, username, mode))
    }

    pub fn get_pending_shares(&self) -> LbResult<Vec<File>> {
        self.rt.block_on(self.lb.get_pending_shares())
    }

    pub fn get_pending_share_files(&self) -> LbResult<Vec<File>> {
        self.rt.block_on(self.lb.get_pending_share_files())
    }

    pub fn delete_pending_share(&self, id: &Uuid) -> LbResult<()> {
        self.rt.block_on(async { self.lb.reject_share(id).await })
    }

    pub fn create_link_at_path(&self, path_and_name: &str, target_id: Uuid) -> LbResult<File> {
        self.rt
            .block_on(self.lb.create_link_at_path(path_and_name, target_id))
    }

    pub fn create_at_path(&self, path_and_name: &str) -> LbResult<File> {
        self.rt.block_on(self.lb.create_at_path(path_and_name))
    }

    pub fn get_by_path(&self, path: &str) -> LbResult<File> {
        self.rt.block_on(self.lb.get_by_path(path))
    }

    pub fn get_path_by_id(&self, id: Uuid) -> LbResult<String> {
        self.rt.block_on(self.lb.get_path_by_id(id))
    }

    pub fn list_paths(&self, filter: Option<Filter>) -> LbResult<Vec<String>> {
        self.rt.block_on(self.lb.list_paths(filter))
    }

    pub fn get_local_changes(&self) -> LbResult<Vec<Uuid>> {
        Ok(self.rt.block_on(self.lb.local_changes()))
    }

    pub fn calculate_work(&self) -> LbResult<SyncStatus> {
        self.rt.block_on(self.lb.calculate_work())
    }

    pub fn sync(&self, f: Option<Box<dyn Fn(SyncProgress) + Send>>) -> LbResult<SyncStatus> {
        self.rt.block_on(self.lb.sync(f))
    }

    pub fn get_last_synced(&self) -> LbResult<i64> {
        self.rt.block_on(async {
            let tx = self.lb.ro_tx().await;
            let db = tx.db();
            Ok(db.last_synced.get().copied().unwrap_or(0))
        })
    }

    pub fn get_last_synced_human_string(&self) -> LbResult<String> {
        self.rt.block_on(self.lb.get_last_synced_human())
    }

    pub fn get_timestamp_human_string(&self, timestamp: i64) -> String {
        self.lb.get_timestamp_human_string(timestamp)
    }

    pub fn suggested_docs(&self, settings: RankingWeights) -> LbResult<Vec<Uuid>> {
        self.rt.block_on(self.lb.suggested_docs(settings))
    }

    pub fn clear_suggested(&self) -> LbResult<()> {
        self.rt.block_on(self.lb.clear_suggested())
    }

    pub fn clear_suggested_id(&self, target_id: Uuid) -> LbResult<()> {
        self.rt.block_on(self.lb.clear_suggested_id(target_id))
    }

    // TODO: examine why the old get_usage does a bunch of things
    pub fn get_usage(&self) -> LbResult<UsageMetrics> {
        self.rt.block_on(self.lb.get_usage())
    }

    pub fn get_uncompressed_usage_breakdown(&self) -> LbResult<HashMap<Uuid, usize>> {
        self.rt.block_on(self.lb.get_uncompressed_usage_breakdown())
    }

    pub fn get_uncompressed_usage(&self) -> LbResult<UsageItemMetric> {
        self.rt.block_on(self.lb.get_uncompressed_usage())
    }

    pub fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> LbResult<()> {
        self.rt
            .block_on(self.lb.import_files(sources, dest, update_status))
    }

    pub fn export_files(
        &self, id: Uuid, dest: PathBuf, edit: bool,
        export_progress: &Option<Box<dyn Fn(ExportFileInfo)>>,
    ) -> LbResult<()> {
        self.rt
            .block_on(self.lb.export_file(id, dest, edit, export_progress))
    }

    pub fn search_file_paths(&self, input: &str) -> LbResult<Vec<SearchResult>> {
        self.rt
            .block_on(async { self.lb.search(input, SearchConfig::Paths).await })
    }

    pub fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<Vec<SearchResult>> {
        self.rt.block_on(self.lb.search(input, cfg))
    }

    pub fn validate(&self) -> LbResult<Vec<Warning>> {
        self.rt.block_on(self.lb.test_repo_integrity())
    }

    pub fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()> {
        self.rt
            .block_on(self.lb.upgrade_account_stripe(account_tier))
    }

    pub fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()> {
        self.rt.block_on(
            self.lb
                .upgrade_account_google_play(purchase_token, account_id),
        )
    }

    pub fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()> {
        self.rt.block_on(
            self.lb
                .upgrade_account_app_store(original_transaction_id, app_account_token),
        )
    }

    pub fn cancel_subscription(&self) -> LbResult<()> {
        self.rt.block_on(self.lb.cancel_subscription())
    }

    pub fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>> {
        self.rt.block_on(self.lb.get_subscription_info())
    }

    pub fn delete_account(&self) -> LbResult<()> {
        self.rt.block_on(self.lb.delete_account())
    }

    pub fn admin_disappear_account(&self, username: &str) -> LbResult<()> {
        self.rt.block_on(self.lb.disappear_account(username))
    }

    pub fn admin_disappear_file(&self, id: Uuid) -> LbResult<()> {
        self.rt.block_on(self.lb.disappear_file(id))
    }

    pub fn admin_list_users(&self, filter: Option<AccountFilter>) -> LbResult<Vec<Username>> {
        self.rt.block_on(self.lb.list_users(filter))
    }

    pub fn admin_get_account_info(&self, identifier: AccountIdentifier) -> LbResult<AccountInfo> {
        self.rt.block_on(self.lb.get_account_info(identifier))
    }

    pub fn admin_validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
        self.rt.block_on(self.lb.validate_account(username))
    }

    pub fn admin_validate_server(&self) -> LbResult<AdminValidateServer> {
        self.rt.block_on(self.lb.validate_server())
    }

    pub fn admin_file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse> {
        self.rt.block_on(self.lb.file_info(id))
    }

    pub fn admin_rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        self.rt.block_on(self.lb.rebuild_index(index))
    }

    pub fn admin_set_user_tier(&self, username: &str, info: AdminSetUserTierInfo) -> LbResult<()> {
        self.rt.block_on(self.lb.set_user_tier(username, info))
    }

    pub fn subscribe(&self) -> Receiver<Event> {
        self.lb.subscribe()
    }

    pub fn status(&self) -> Status {
        self.rt.block_on(self.lb.status())
    }

    pub fn debug_info(&self, os_info: String) -> String {
        self.rt
            .block_on(self.lb.debug_info(os_info))
            .unwrap_or_else(|e| format!("failed to produce debug info: {:?}", e.to_string()))
    }

    pub fn write_panic_to_file(&self, error_header: String, bt: String) -> LbResult<String> {
        self.rt
            .block_on(self.lb.write_panic_to_file(error_header, bt))
    }
}
