use std::sync::Arc;

use tokio::runtime::Runtime;

use crate::model::{account::Account, core_config::Config, errors::LbResult};

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

    pub fn get_account(&self) -> LbResult<Account> {
        self.lb.get_account()
    }

    pub fn get_config(&self) -> Config {
        self.lb.config.clone()
    }

    pub fn create_file(&self, name: &str, parent: Uuid, file_type: FileType) -> LbResult<File> {
        self.rt
            .block_on(self.lb.create_file(name, parent, file_type))
    }

    pub fn write_document(&self, id: Uuid, content: &[u8]) -> LbResult<()> {
        self.rt.block_on(self.write_document(id, content))
    }

    pub fn get_root(&self) -> LbResult<File> {
        self.rt.block_on(self.lb.root())
    }

    pub fn get_children(&self, id: Uuid) -> Result<Vec<File>, UnexpectedError> {
        self.rt.block_on(self.lb.get_children(id))
    }

    pub fn get_and_get_children_recursively(&self, id: Uuid) -> LbResult<Vec<File>> {
        self.rt
            .block_on(self.lb.get_and_get_children_recursively(id))
    }

    pub fn get_file_by_id(&self, id: Uuid) -> LbResult<File> {
        self.rt.block_on(self.lb.get_file_by_id(id))
    }

    pub fn delete_file(&self, id: Uuid) -> LbResult<()> {
        self.rt.block_on(self.lb.delete(id))
    }

    pub fn read_document(&self, id: Uuid) -> LbResult<DecryptedDocument> {
        self.rt.block_on(self.lb.read_document(id))
    }

    pub fn list_metadatas(&self) -> Result<Vec<File>, UnexpectedError> {
        self.rt.block_on(self.lb.list_metadatas())
    }

    pub fn rename_file(&self, id: Uuid, new_name: &str) -> LbResult<()> {
        self.rt.block_on(self.lb.rename_file(id, new_name))
    }

    pub fn move_file(&self, id: Uuid, new_parent: Uuid) -> LbResult<()> {
        self.rt.block_on(self.lb.move_file(id, new_parent))
    }

    pub fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()> {
        self.in_tx(|s| s.share_file(id, username, mode))
            .expected_errs(&[
                CoreError::RootModificationInvalid,
                CoreError::FileNonexistent,
                CoreError::ShareAlreadyExists,
                CoreError::LinkInSharedFolder,
                CoreError::InsufficientPermission,
            ])
    }

    pub fn get_pending_shares(&self) -> Result<Vec<File>, UnexpectedError> {
        Ok(self.in_tx(|s| s.get_pending_shares())?)
    }

    pub fn delete_pending_share(&self, id: Uuid) -> LbResult<()> {
        self.in_tx(|s| {
            let pk = s.get_public_key()?;
            s.delete_share(&id, Some(pk))
        })
        .expected_errs(&[CoreError::FileNonexistent, CoreError::ShareNonexistent])
    }

    pub fn create_link_at_path(&self, path_and_name: &str, target_id: Uuid) -> LbResult<File> {
        self.in_tx(|s| s.create_link_at_path(path_and_name, target_id))
            .expected_errs(&[
                CoreError::FileNotFolder,
                CoreError::PathContainsEmptyFileName,
                CoreError::PathTaken,
                CoreError::FileNameTooLong,
                CoreError::LinkInSharedFolder,
                CoreError::LinkTargetIsOwned,
                CoreError::LinkTargetNonexistent,
                CoreError::MultipleLinksToSameFile,
            ])
    }

    pub fn create_at_path(&self, path_and_name: &str) -> LbResult<File> {
        self.in_tx(|s| s.create_at_path(path_and_name))
            .expected_errs(&[
                CoreError::FileNotFolder,
                CoreError::InsufficientPermission,
                CoreError::PathContainsEmptyFileName,
                CoreError::FileNameTooLong,
                CoreError::PathTaken,
                CoreError::RootNonexistent,
            ])
    }

    pub fn get_by_path(&self, path: &str) -> LbResult<File> {
        self.in_tx(|s| s.get_by_path(path))
            .expected_errs(&[CoreError::FileNonexistent])
    }

    pub fn get_path_by_id(&self, id: Uuid) -> Result<String, UnexpectedError> {
        Ok(self.in_tx(|s| s.get_path_by_id(id))?)
    }

    pub fn list_paths(&self, filter: Option<Filter>) -> Result<Vec<String>, UnexpectedError> {
        Ok(self.in_tx(|s| s.list_paths(filter))?)
    }

    pub fn get_local_changes(&self) -> Result<Vec<Uuid>, UnexpectedError> {
        Ok(self.in_tx(|s| {
            Ok(s.db
                .local_metadata
                .get()
                .keys()
                .copied()
                .collect::<Vec<Uuid>>())
        })?)
    }

    pub fn calculate_work(&self) -> LbResult<SyncStatus> {
        self.in_tx(|s| s.calculate_work())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    // todo: expose work calculated (return value)
    pub fn sync(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> LbResult<SyncStatus> {
        SyncContext::sync(self, f).expected_errs(&[
            CoreError::ServerUnreachable, // todo already syncing?
            CoreError::ClientUpdateRequired,
            CoreError::UsageIsOverDataCap,
        ])
    }

    pub fn get_last_synced(&self) -> Result<i64, UnexpectedError> {
        Ok(self.in_tx(|s| Ok(s.db.last_synced.get().copied().unwrap_or(0)))?)
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
        Ok(self.in_tx(|s| s.suggested_docs(settings))?)
    }

    pub fn get_usage(&self) -> LbResult<UsageMetrics> {
        let acc = self.get_account()?;
        let s = self.inner.lock().unwrap();
        let client = s.client.clone();
        drop(s);
        let usage = client.request(&acc, GetUsageRequest {})?;
        self.in_tx(|s| s.get_usage(usage))
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    pub fn get_uncompressed_usage_breakdown(
        &self,
    ) -> Result<HashMap<Uuid, usize>, UnexpectedError> {
        Ok(self.in_tx(|s| s.get_uncompressed_usage_breakdown())?)
    }

    pub fn get_uncompressed_usage(&self) -> LbResult<UsageItemMetric> {
        // todo the errors here are wrong this doesn't talk to the server
        self.in_tx(|s| s.get_uncompressed_usage())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    pub fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> LbResult<()> {
        self.in_tx(|s| s.import_files(sources, dest, update_status))
            .expected_errs(&[
                CoreError::DiskPathInvalid,
                CoreError::FileNonexistent,
                CoreError::FileNotFolder,
                CoreError::FileNameTooLong,
            ])
    }

    pub fn export_file(
        &self, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ExportFileInfo)>>,
    ) -> LbResult<()> {
        self.in_tx(|s| s.export_file(id, destination, edit, export_progress))
            .expected_errs(&[
                CoreError::FileNonexistent,
                CoreError::DiskPathInvalid,
                CoreError::DiskPathTaken,
            ])
    }

    pub fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, UnexpectedError> {
        Ok(self.in_tx(|s| s.search_file_paths(input))?)
    }

    pub fn start_search(&self, search_type: SearchType) -> StartSearchInfo {
        let (search_tx, search_rx) = channel::unbounded::<SearchRequest>();
        let (results_tx, results_rx) = channel::unbounded::<SearchResult>();

        let core = self.clone();

        let results_tx_c = results_tx.clone();

        thread::spawn(move || {
            if let Err(err) = core.in_tx(|s| s.start_search(search_type, results_tx, search_rx)) {
                let _ = results_tx_c.send(SearchResult::Error(err.into()));
            }
        });

        StartSearchInfo { search_tx, results_rx }
    }

    pub fn validate(&self) -> Result<Vec<Warning>, TestRepoError> {
        self.in_tx(|s| Ok(s.test_repo_integrity()))
            .map_err(TestRepoError::Core)?
    }

    pub fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()> {
        self.in_tx(|s| s.upgrade_account_stripe(account_tier))
            .expected_errs(&[
                CoreError::OldCardDoesNotExist,
                CoreError::CardInvalidNumber,
                CoreError::CardInvalidExpYear,
                CoreError::CardInvalidExpMonth,
                CoreError::CardInvalidCvc,
                CoreError::AlreadyPremium,
                CoreError::ServerUnreachable,
                CoreError::CardDecline,
                CoreError::CardInsufficientFunds,
                CoreError::TryAgain,
                CoreError::CardNotSupported,
                CoreError::CardExpired,
                CoreError::CurrentUsageIsMoreThanNewTier,
                CoreError::ExistingRequestPending,
                CoreError::ClientUpdateRequired,
            ])
    }

    pub fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()> {
        self.in_tx(|s| s.upgrade_account_google_play(purchase_token, account_id))
            .expected_errs(&[
                CoreError::AlreadyPremium,
                CoreError::InvalidAuthDetails,
                CoreError::ExistingRequestPending,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
                CoreError::AppStoreAccountAlreadyLinked,
            ])
    }

    pub fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()> {
        self.in_tx(|s| s.upgrade_account_app_store(original_transaction_id, app_account_token))
            .expected_errs(&[
                CoreError::AlreadyPremium,
                CoreError::InvalidPurchaseToken,
                CoreError::ExistingRequestPending,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
                CoreError::AppStoreAccountAlreadyLinked,
            ])
    }

    pub fn cancel_subscription(&self) -> LbResult<()> {
        self.in_tx(|s| s.cancel_subscription()).expected_errs(&[
            CoreError::NotPremium,
            CoreError::AlreadyCanceled,
            CoreError::UsageIsOverFreeTierDataCap,
            CoreError::ExistingRequestPending,
            CoreError::CannotCancelSubscriptionForAppStore,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    pub fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>> {
        self.in_tx(|s| s.get_subscription_info())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    pub fn delete_account(&self) -> LbResult<()> {
        self.in_tx(|s| s.delete_account())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    pub fn admin_disappear_account(&self, username: &str) -> LbResult<()> {
        self.in_tx(|s| s.disappear_account(username))
            .expected_errs(&[
                CoreError::UsernameNotFound,
                CoreError::InsufficientPermission,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
            ])
    }

    pub fn admin_disappear_file(&self, id: Uuid) -> LbResult<()> {
        self.in_tx(|s| s.disappear_file(id)).expected_errs(&[
            CoreError::FileNonexistent,
            CoreError::InsufficientPermission,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    pub fn admin_list_users(&self, filter: Option<AccountFilter>) -> LbResult<Vec<Username>> {
        self.in_tx(|s| s.list_users(filter)).expected_errs(&[
            CoreError::InsufficientPermission,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    pub fn admin_get_account_info(&self, identifier: AccountIdentifier) -> LbResult<AccountInfo> {
        self.in_tx(|s| s.get_account_info(identifier))
            .expected_errs(&[
                CoreError::UsernameNotFound,
                CoreError::InsufficientPermission,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
            ])
    }

    pub fn admin_validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
        self.in_tx(|s| s.validate_account(username))
            .expected_errs(&[
                CoreError::UsernameNotFound,
                CoreError::InsufficientPermission,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
            ])
    }

    pub fn admin_validate_server(&self) -> LbResult<AdminValidateServer> {
        self.in_tx(|s| s.validate_server()).expected_errs(&[
            CoreError::InsufficientPermission,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    pub fn admin_file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse> {
        self.in_tx(|s| s.file_info(id)).expected_errs(&[
            CoreError::FileNonexistent,
            CoreError::InsufficientPermission,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    pub fn admin_rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        self.in_tx(|s| s.rebuild_index(index)).expected_errs(&[
            CoreError::InsufficientPermission,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    pub fn admin_set_user_tier(&self, username: &str, info: AdminSetUserTierInfo) -> LbResult<()> {
        self.in_tx(|s| s.set_user_tier(username, info))
            .expected_errs(&[
                CoreError::UsernameNotFound,
                CoreError::InsufficientPermission,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
                CoreError::ExistingRequestPending,
            ])
    }

    pub fn debug_info(&self, os_info: String) -> String {
        match self.in_tx(|s| s.debug_info(os_info)) {
            Ok(debug_info) => debug_info,
            Err(e) => format!("failed to produce debug info: {:?}", e.to_string()),
        }
    }
}
