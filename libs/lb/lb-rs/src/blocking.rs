use std::{path::PathBuf, sync::Arc};

use tokio::runtime::Runtime;
use uuid::Uuid;

use crate::{
    logic::{crypto::DecryptedDocument, path_ops::Filter},
    model::{
        account::Account,
        core_config::Config,
        errors::LbResult,
        file::{File, ShareMode},
        file_metadata::FileType,
    },
    service::{
        import_export::{ExportFileInfo, ImportStatus},
        sync::{SyncProgress, SyncStatus},
        usage::UsageItemMetric,
    },
};

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
        self.export_account_qr()
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

    pub fn read_document(&self, id: Uuid) -> LbResult<DecryptedDocument> {
        self.rt.block_on(self.lb.read_document(id))
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

    pub fn delete_pending_share(&self, id: &Uuid) -> LbResult<()> {
        self.rt.block_on(async {
            let pk = self.lb.get_pk()?;
            self.lb.delete_share(id, Some(pk)).await
        })
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
        self.rt.block_on(async {
            let tx = self.lb.ro_tx().await;
            let db = tx.db();
            Ok(db.local_metadata.get().keys().copied().collect())
        })
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

    // todo: pub fn get_last_synced_human_string(&self) -> Result<String, UnexpectedError> {
    // todo:     let last_synced = self.get_last_synced()?;

    // todo:     Ok(if last_synced != 0 {
    // todo:         Duration::milliseconds(clock::get_time().0 - last_synced)
    // todo:             .format_human()
    // todo:             .to_string()
    // todo:     } else {
    // todo:         "never".to_string()
    // todo:     })
    // todo: }

    // todo: pub fn suggested_docs(&self, settings: RankingWeights) -> Result<Vec<Uuid>, UnexpectedError> {
    // todo:     Ok(self.in_tx(|s| s.suggested_docs(settings))?)
    // todo: }

    // todo: pub fn get_usage(&self) -> LbResult<UsageMetrics> {
    // todo:     let acc = self.get_account()?;
    // todo:     let s = self.inner.lock().unwrap();
    // todo:     let client = s.client.clone();
    // todo:     drop(s);
    // todo:     let usage = client.request(&acc, GetUsageRequest {})?;
    // todo:     self.in_tx(|s| s.get_usage(usage))
    // todo:         .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    // todo: }

    // todo: pub fn get_uncompressed_usage_breakdown(
    // todo:     &self,
    // todo: ) -> Result<HashMap<Uuid, usize>, UnexpectedError> {
    // todo:     Ok(self.in_tx(|s| s.get_uncompressed_usage_breakdown())?)
    // todo: }

    // todo: pub fn get_uncompressed_usage(&self) -> LbResult<UsageItemMetric> {
    // todo:     self.rt.block_on(self.lb.get_uncompressed_usage())
    // todo: }

    // todo: pub fn import_files<F: Fn(ImportStatus)>(
    // todo:     &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    // todo: ) -> LbResult<()> {
    // todo:     self.rt.block_on(self.lb.import_files(sources, dest, update_status))
    // todo: }

    // todo: pub fn export_file(
    // todo:     &self, id: Uuid, destination: PathBuf, edit: bool,
    // todo:     export_progress: &Option<Box<dyn Fn(ExportFileInfo)>>,
    // todo: ) -> LbResult<()> {
    // todo:     self.rt.block_on(self.lb.export_file(id, destination, edit, export_progress))
    // todo: }

    // todo: pub fn search_file_paths(&self, input: &str) -> LbResult<Vec<SearchResultItem>> {
    // todo:     Ok(self.in_tx(|s| s.search_file_paths(input))?)
    // todo: }

    // todo: pub fn start_search(&self, search_type: SearchType) -> StartSearchInfo {
    // todo:     let (search_tx, search_rx) = channel::unbounded::<SearchRequest>();
    // todo:     let (results_tx, results_rx) = channel::unbounded::<SearchResult>();

    // todo:     let core = self.clone();

    // todo:     let results_tx_c = results_tx.clone();

    // todo:     thread::spawn(move || {
    // todo:         if let Err(err) = core.in_tx(|s| s.start_search(search_type, results_tx, search_rx)) {
    // todo:             let _ = results_tx_c.send(SearchResult::Error(err.into()));
    // todo:         }
    // todo:     });

    // todo:     StartSearchInfo { search_tx, results_rx }
    // todo: }

    // todo: pub fn validate(&self) -> Result<Vec<Warning>, TestRepoError> {
    // todo:     self.in_tx(|s| Ok(s.test_repo_integrity()))
    // todo:         .map_err(TestRepoError::Core)?
    // todo: }

    // todo: pub fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()> {
    // todo:     self.in_tx(|s| s.upgrade_account_stripe(account_tier))
    // todo:         .expected_errs(&[
    // todo:             CoreError::OldCardDoesNotExist,
    // todo:             CoreError::CardInvalidNumber,
    // todo:             CoreError::CardInvalidExpYear,
    // todo:             CoreError::CardInvalidExpMonth,
    // todo:             CoreError::CardInvalidCvc,
    // todo:             CoreError::AlreadyPremium,
    // todo:             CoreError::ServerUnreachable,
    // todo:             CoreError::CardDecline,
    // todo:             CoreError::CardInsufficientFunds,
    // todo:             CoreError::TryAgain,
    // todo:             CoreError::CardNotSupported,
    // todo:             CoreError::CardExpired,
    // todo:             CoreError::CurrentUsageIsMoreThanNewTier,
    // todo:             CoreError::ExistingRequestPending,
    // todo:             CoreError::ClientUpdateRequired,
    // todo:         ])
    // todo: }

    // todo: pub fn upgrade_account_google_play(
    // todo:     &self, purchase_token: &str, account_id: &str,
    // todo: ) -> LbResult<()> {
    // todo:     self.in_tx(|s| s.upgrade_account_google_play(purchase_token, account_id))
    // todo:         .expected_errs(&[
    // todo:             CoreError::AlreadyPremium,
    // todo:             CoreError::InvalidAuthDetails,
    // todo:             CoreError::ExistingRequestPending,
    // todo:             CoreError::ServerUnreachable,
    // todo:             CoreError::ClientUpdateRequired,
    // todo:             CoreError::AppStoreAccountAlreadyLinked,
    // todo:         ])
    // todo: }

    // todo: pub fn upgrade_account_app_store(
    // todo:     &self, original_transaction_id: String, app_account_token: String,
    // todo: ) -> LbResult<()> {
    // todo:     self.in_tx(|s| s.upgrade_account_app_store(original_transaction_id, app_account_token))
    // todo:         .expected_errs(&[
    // todo:             CoreError::AlreadyPremium,
    // todo:             CoreError::InvalidPurchaseToken,
    // todo:             CoreError::ExistingRequestPending,
    // todo:             CoreError::ServerUnreachable,
    // todo:             CoreError::ClientUpdateRequired,
    // todo:             CoreError::AppStoreAccountAlreadyLinked,
    // todo:         ])
    // todo: }

    // todo: pub fn cancel_subscription(&self) -> LbResult<()> {
    // todo:     self.in_tx(|s| s.cancel_subscription()).expected_errs(&[
    // todo:         CoreError::NotPremium,
    // todo:         CoreError::AlreadyCanceled,
    // todo:         CoreError::UsageIsOverFreeTierDataCap,
    // todo:         CoreError::ExistingRequestPending,
    // todo:         CoreError::CannotCancelSubscriptionForAppStore,
    // todo:         CoreError::ServerUnreachable,
    // todo:         CoreError::ClientUpdateRequired,
    // todo:     ])
    // todo: }

    // todo: pub fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>> {
    // todo:     self.in_tx(|s| s.get_subscription_info())
    // todo:         .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    // todo: }

    // todo: pub fn delete_account(&self) -> LbResult<()> {
    // todo:     self.in_tx(|s| s.delete_account())
    // todo:         .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    // todo: }

    // todo: pub fn admin_disappear_account(&self, username: &str) -> LbResult<()> {
    // todo:     self.in_tx(|s| s.disappear_account(username))
    // todo:         .expected_errs(&[
    // todo:             CoreError::UsernameNotFound,
    // todo:             CoreError::InsufficientPermission,
    // todo:             CoreError::ServerUnreachable,
    // todo:             CoreError::ClientUpdateRequired,
    // todo:         ])
    // todo: }

    // todo: pub fn admin_disappear_file(&self, id: Uuid) -> LbResult<()> {
    // todo:     self.in_tx(|s| s.disappear_file(id)).expected_errs(&[
    // todo:         CoreError::FileNonexistent,
    // todo:         CoreError::InsufficientPermission,
    // todo:         CoreError::ServerUnreachable,
    // todo:         CoreError::ClientUpdateRequired,
    // todo:     ])
    // todo: }

    // todo: pub fn admin_list_users(&self, filter: Option<AccountFilter>) -> LbResult<Vec<Username>> {
    // todo:     self.in_tx(|s| s.list_users(filter)).expected_errs(&[
    // todo:         CoreError::InsufficientPermission,
    // todo:         CoreError::ServerUnreachable,
    // todo:         CoreError::ClientUpdateRequired,
    // todo:     ])
    // todo: }

    // todo: pub fn admin_get_account_info(&self, identifier: AccountIdentifier) -> LbResult<AccountInfo> {
    // todo:     self.in_tx(|s| s.get_account_info(identifier))
    // todo:         .expected_errs(&[
    // todo:             CoreError::UsernameNotFound,
    // todo:             CoreError::InsufficientPermission,
    // todo:             CoreError::ServerUnreachable,
    // todo:             CoreError::ClientUpdateRequired,
    // todo:         ])
    // todo: }

    // todo: pub fn admin_validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
    // todo:     self.in_tx(|s| s.validate_account(username))
    // todo:         .expected_errs(&[
    // todo:             CoreError::UsernameNotFound,
    // todo:             CoreError::InsufficientPermission,
    // todo:             CoreError::ServerUnreachable,
    // todo:             CoreError::ClientUpdateRequired,
    // todo:         ])
    // todo: }

    // todo: pub fn admin_validate_server(&self) -> LbResult<AdminValidateServer> {
    // todo:     self.in_tx(|s| s.validate_server()).expected_errs(&[
    // todo:         CoreError::InsufficientPermission,
    // todo:         CoreError::ServerUnreachable,
    // todo:         CoreError::ClientUpdateRequired,
    // todo:     ])
    // todo: }

    // todo: pub fn admin_file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse> {
    // todo:     self.in_tx(|s| s.file_info(id)).expected_errs(&[
    // todo:         CoreError::FileNonexistent,
    // todo:         CoreError::InsufficientPermission,
    // todo:         CoreError::ServerUnreachable,
    // todo:         CoreError::ClientUpdateRequired,
    // todo:     ])
    // todo: }

    // todo: pub fn admin_rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
    // todo:     self.in_tx(|s| s.rebuild_index(index)).expected_errs(&[
    // todo:         CoreError::InsufficientPermission,
    // todo:         CoreError::ServerUnreachable,
    // todo:         CoreError::ClientUpdateRequired,
    // todo:     ])
    // todo: }

    // todo: pub fn admin_set_user_tier(&self, username: &str, info: AdminSetUserTierInfo) -> LbResult<()> {
    // todo:     self.in_tx(|s| s.set_user_tier(username, info))
    // todo:         .expected_errs(&[
    // todo:             CoreError::UsernameNotFound,
    // todo:             CoreError::InsufficientPermission,
    // todo:             CoreError::ServerUnreachable,
    // todo:             CoreError::ClientUpdateRequired,
    // todo:             CoreError::ExistingRequestPending,
    // todo:         ])
    // todo: }

    // todo: pub fn debug_info(&self, os_info: String) -> String {
    // todo:     match self.in_tx(|s| s.debug_info(os_info)) {
    // todo:         Ok(debug_info) => debug_info,
    // todo:         Err(e) => format!("failed to produce debug info: {:?}", e.to_string()),
    // todo:     }
    // todo: }
}
