use crate::model::core_config::Config;
use crate::model::errors::core_err_unexpected;
use crate::model::{
    account::{Account, Username},
    api::{
        AccountFilter, AccountIdentifier, AccountInfo, AdminFileInfoResponse, AdminSetUserTierInfo,
        AdminValidateAccount, AdminValidateServer, ServerIndex, StripeAccountTier,
        SubscriptionInfo,
    },
    crypto::DecryptedDocument,
    errors::{LbErr, Warning},
    file::{File, ShareMode},
    file_metadata::{DocumentHmac, FileType},
    path_ops::Filter,
};
use crate::rpc::{call_rpc, recv_rpc_response, send_rpc_request, Method};
use crate::service::events::EventSubs;
use crate::service::keychain::Keychain;
use crate::service::sync::{SyncProgress, SyncStatus};
use crate::service::{
    activity::RankingWeights,
    events::Event,
    import_export::{ExportFileInfo, ImportStatus},
    usage::{UsageItemMetric, UsageMetrics},
};
use crate::subscribers::search::{SearchConfig, SearchIndex, SearchResult};
use crate::subscribers::status::Status;
use crate::LbResult;
use crate::Uuid;
use std::collections::HashMap;
use std::net::SocketAddrV4;
use std::path::{Path, PathBuf};
use tokio::net::TcpStream;
use tokio::sync::broadcast::{self, Receiver};

#[derive(Clone)]
pub struct LbClient {
    pub addr: SocketAddrV4,
    pub events: EventSubs,
}

impl LbClient {
    pub async fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        let args = (username.to_string(), api_url.to_string(), welcome_doc);
        call_rpc(self.addr, Method::CreateAccount, args).await
    }

    pub async fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        let args = (key.to_string(), api_url.map(|s| s.to_string()));
        call_rpc(self.addr, Method::ImportAccount, args).await
    }

    pub async fn import_account_private_key_v1(&self, account: Account) -> LbResult<Account> {
        let args = account;
        call_rpc(self.addr, Method::ImportAccountPrivateKeyV1, args).await
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn export_account_private_key(&self) -> LbResult<String> {
        call_rpc(self.addr, Method::ExportAccountPrivateKey, ()).await
    }

    pub(crate) async fn export_account_private_key_v1(&self) -> LbResult<String> {
        call_rpc(self.addr, Method::ExportAccountPrivateKeyV1, ()).await
    }

    pub(crate) async fn export_account_private_key_v2(&self) -> LbResult<String> {
        call_rpc(self.addr, Method::ExportAccountPrivateKeyV2, ()).await
    }

    pub async fn export_account_phrase(&self) -> LbResult<String> {
        call_rpc(self.addr, Method::ExportAccountPhrase, ()).await
    }

    pub async fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        call_rpc(self.addr, Method::ExportAccountQr, ()).await
    }

    pub async fn delete_account(&self) -> LbResult<()> {
        call_rpc(self.addr, Method::DeleteAccount, ()).await
    }

    pub async fn suggested_docs(&self, settings: RankingWeights) -> LbResult<Vec<Uuid>> {
        let args = settings;
        call_rpc(self.addr, Method::SuggestedDocs, args).await
    }

    pub async fn disappear_account(&self, username: &str) -> LbResult<()> {
        let args = username.to_string();
        call_rpc(self.addr, Method::DisappearAccount, args).await
    }

    pub async fn disappear_file(&self, id: Uuid) -> LbResult<()> {
        let args = id;
        call_rpc(self.addr, Method::DisappearFile, args).await
    }

    pub async fn list_users(&self, filter: Option<AccountFilter>) -> LbResult<Vec<Username>> {
        let args = filter;
        call_rpc(self.addr, Method::ListUsers, args).await
    }

    pub async fn get_account_info(&self, identifier: AccountIdentifier) -> LbResult<AccountInfo> {
        let args = identifier;
        call_rpc(self.addr, Method::GetAccountInfo, args).await
    }

    pub async fn validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
        let args = username.to_string();
        call_rpc(self.addr, Method::ValidateAccount, args).await
    }

    pub async fn validate_server(&self) -> LbResult<AdminValidateServer> {
        call_rpc(self.addr, Method::ValidateServer, ()).await
    }

    pub async fn file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse> {
        let args = id;
        call_rpc(self.addr, Method::FileInfo, args).await
    }

    pub async fn rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        let args = index;
        call_rpc(self.addr, Method::RebuildIndex, args).await
    }

    pub async fn build_index(&self) -> LbResult<()> {
        call_rpc(self.addr, Method::BuildIndex, ()).await
    }

    pub async fn set_user_tier(&self, username: &str, info: AdminSetUserTierInfo) -> LbResult<()> {
        let args = (username.to_string(), info);
        call_rpc(self.addr, Method::SetUserTier, args).await
    }

    pub async fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()> {
        let args = account_tier;
        call_rpc(self.addr, Method::UpgradeAccountStripe, args).await
    }

    pub async fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()> {
        let args = (purchase_token.to_string(), account_id.to_string());
        call_rpc(self.addr, Method::UpgradeAccountGooglePlay, args).await
    }

    pub async fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()> {
        let args = (original_transaction_id, app_account_token);
        call_rpc(self.addr, Method::UpgradeAccountAppStore, args).await
    }

    pub async fn cancel_subscription(&self) -> LbResult<()> {
        call_rpc(self.addr, Method::CancelSubscription, ()).await
    }

    pub async fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>> {
        call_rpc(self.addr, Method::GetSubscriptionInfo, ()).await
    }

    pub async fn debug_info(&self, os_info: String) -> LbResult<String> {
        let args = os_info;
        call_rpc(self.addr, Method::DebugInfo, args).await
    }

    pub async fn read_document(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<DecryptedDocument> {
        let args = (id, user_activity);
        call_rpc(self.addr, Method::ReadDocument, args).await
    }

    pub async fn write_document(&self, id: Uuid, content: &[u8]) -> LbResult<()> {
        let args = (id, content);
        call_rpc(self.addr, Method::WriteDocument, args).await
    }

    pub async fn read_document_with_hmac(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        let args = (id, user_activity);
        call_rpc(self.addr, Method::ReadDocumentWithHmac, args).await
    }

    pub async fn safe_write(
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        let args = (id, old_hmac, content);
        call_rpc(self.addr, Method::SafeWrite, args).await
    }

    pub async fn subscribe(&self) -> Receiver<Event> {
        let (local_tx, local_rx) = broadcast::channel(16);
        let addr = self.addr;

        tokio::spawn(async move {
            match TcpStream::connect(addr).await {
                Ok(mut stream) => {
                    if let Err(e) = send_rpc_request(&mut stream, Method::Subscribe, &()).await {
                        eprintln!("Subscribe send error: {:?}", e);
                        return;
                    }
                    loop {
                        match recv_rpc_response::<Event>(&mut stream).await {
                            Ok(event) => {
                                let _ = local_tx.send(event);
                            }
                            Err(e) => {
                                eprintln!("Subscribe receive error: {:?}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Subscribe connection error: {:?}", e);
                }
            }
        });

        local_rx
    }

    pub async fn create_file(
        &self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> LbResult<File> {
        let args = (name.to_string(), *parent, file_type);
        call_rpc(self.addr, Method::CreateFile, args).await
    }

    pub async fn rename_file(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        let args = (*id, new_name.to_string());
        call_rpc(self.addr, Method::RenameFile, args).await
    }

    pub async fn move_file(&self, id: &Uuid, new_parent: &Uuid) -> LbResult<()> {
        let args = (*id, *new_parent);
        call_rpc(self.addr, Method::MoveFile, args).await
    }

    pub async fn delete(&self, id: &Uuid) -> LbResult<()> {
        let args = id;
        call_rpc(self.addr, Method::Delete, args).await
    }

    pub async fn root(&self) -> LbResult<File> {
        call_rpc(self.addr, Method::Root, ()).await
    }

    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        call_rpc(self.addr, Method::ListMetadatas, ()).await
    }

    pub async fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let args = id;
        call_rpc(self.addr, Method::GetChildren, args).await
    }

    pub async fn get_and_get_children_recursively(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let args = id;
        call_rpc(self.addr, Method::GetAndGetChildrenRecursively, args).await
    }

    pub async fn get_file_by_id(&self, id: Uuid) -> LbResult<File> {
        let args = id;
        call_rpc(self.addr, Method::GetFileById, args).await
    }

    pub async fn local_changes(&self) -> Vec<Uuid> {
        call_rpc(self.addr, Method::LocalChanges, ()).await.unwrap()
    }

    pub async fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &Option<F>,
    ) -> LbResult<()> {
        let source_paths: Vec<String> = sources
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let args = bincode::serialize(&(source_paths, dest)).map_err(core_err_unexpected)?;
        call_rpc(self.addr, Method::ImportFiles, args).await
    }

    pub async fn export_file<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, dest: PathBuf, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()> {
        let args = (id, dest.clone(), edit);
        call_rpc(self.addr, Method::ExportFile, args).await
    }

    pub async fn export_file_recursively<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, disk_path: &Path, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()> {
        let args = (id, disk_path.to_path_buf(), edit);
        call_rpc(self.addr, Method::ExportFileRecursively, args).await
    }

    pub async fn test_repo_integrity(&self) -> LbResult<Vec<Warning>> {
        call_rpc(self.addr, Method::TestRepoIntegrity, ()).await
    }

    pub async fn get_account(&self) -> LbResult<Account> {
        call_rpc(self.addr, Method::GetAccount, ()).await
    }

    pub async fn create_link_at_path(&self, path: &str, target_id: Uuid) -> LbResult<File> {
        let args = (path.to_string(), target_id);
        call_rpc(self.addr, Method::CreateLinkAtPath, args).await
    }

    pub async fn create_at_path(&self, path: &str) -> LbResult<File> {
        let args = path.to_string();
        call_rpc(self.addr, Method::CreateAtPath, args).await
    }

    pub async fn get_by_path(&self, path: &str) -> LbResult<File> {
        let args = path.to_string();
        call_rpc(self.addr, Method::GetByPath, args).await
    }

    pub async fn get_path_by_id(&self, id: Uuid) -> LbResult<String> {
        let args = id;
        call_rpc(self.addr, Method::GetPathById, args).await
    }

    pub async fn list_paths(&self, filter: Option<Filter>) -> LbResult<Vec<String>> {
        let args = filter;
        call_rpc(self.addr, Method::ListPaths, args).await
    }

    pub async fn list_paths_with_ids(
        &self, filter: Option<Filter>,
    ) -> LbResult<Vec<(Uuid, String)>> {
        let args = filter;
        call_rpc(self.addr, Method::ListPathsWithIds, args).await
    }

    pub async fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()> {
        let args = (id, username.to_string(), mode);
        call_rpc(self.addr, Method::ShareFile, args).await
    }

    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>> {
        call_rpc(self.addr, Method::GetPendingShares, ()).await
    }

    pub async fn reject_share(&self, id: &Uuid) -> Result<(), LbErr> {
        let args = id;
        call_rpc(self.addr, Method::RejectShare, args).await
    }

    pub async fn calculate_work(&self) -> LbResult<SyncStatus> {
        call_rpc(self.addr, Method::CalculateWork, ()).await
    }

    pub async fn sync(&self, f: Option<Box<dyn Fn(SyncProgress) + Send>>) -> LbResult<SyncStatus> {
        call_rpc(self.addr, Method::Sync, ()).await
    }

    pub async fn get_last_synced_human(&self) -> LbResult<String> {
        call_rpc(self.addr, Method::GetLastSyncedHuman, ()).await
    }

    pub async fn get_timestamp_human_string(&self, timestamp: i64) -> String {
        let args = timestamp;
        call_rpc(self.addr, Method::GetTimestampHumanString, args)
            .await
            .unwrap()
    }

    pub async fn get_usage(&self) -> LbResult<UsageMetrics> {
        call_rpc(self.addr, Method::GetUsage, ()).await
    }

    pub async fn get_uncompressed_usage_breakdown(&self) -> LbResult<HashMap<Uuid, usize>> {
        call_rpc(self.addr, Method::GetUncompressedUsageBreakdown, ()).await
    }

    pub async fn get_uncompressed_usage(&self) -> LbResult<UsageItemMetric> {
        call_rpc(self.addr, Method::GetUncompressedUsage, ()).await
    }

    pub async fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<Vec<SearchResult>> {
        let args = (input.to_string(), cfg);
        call_rpc(self.addr, Method::Search, args).await
    }

    pub async fn status(&self) -> Status {
        call_rpc(self.addr, Method::Status, ()).await.unwrap()
    }

    pub async fn get_config(&self) -> Config {
        call_rpc(self.addr, Method::GetConfig, ()).await.unwrap()
    }

    pub async fn get_last_synced(&self) -> LbResult<i64> {
        call_rpc(self.addr, Method::GetLastSynced, ()).await
    }

    pub async fn get_search(&self) -> SearchIndex {
        call_rpc(self.addr, Method::GetSearch, ()).await.unwrap()
    }

    pub async fn get_keychain(&self) -> Keychain {
        call_rpc(self.addr, Method::GetKeychain, ()).await.unwrap()
    }
}
