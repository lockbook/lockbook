use crate::model::core_config::Config;
use crate::model::{
    account::{Account, Username},
    api::{
        AccountFilter, AccountIdentifier, AccountInfo, AdminFileInfoResponse, AdminSetUserTierInfo,
        AdminValidateAccount, AdminValidateServer, ServerIndex, StripeAccountTier,
        SubscriptionInfo,
    },
    crypto::DecryptedDocument,
    errors::{LbErr, LbErrKind, Warning},
    file::{File, ShareMode},
    file_metadata::{DocumentHmac, FileType},
    path_ops::Filter,
};
use crate::service::events::EventSubs;
use crate::service::keychain::Keychain;
use crate::service::{
    activity::RankingWeights,
    events::Event,
    import_export::{ExportFileInfo, ImportStatus},
    sync::{SyncProgress, SyncStatus},
    usage::{UsageItemMetric, UsageMetrics},
};
use crate::subscribers::search::{SearchConfig, SearchIndex, SearchResult};
use crate::subscribers::status::Status;
use crate::{lb_client::LbClient, LbResult, LbServer};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::{Path, PathBuf};
use tokio::net::TcpListener;
use tokio::sync::broadcast::Receiver;
use uuid::Uuid;

#[derive(Clone)]
pub enum Lb {
    Direct(LbServer),
    Network(LbClient),
}

impl Lb {
    pub async fn init(config: Config) -> LbResult<Self> {
        match config.rpc_port {
            Some(port) => {
                let socket = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
                match TcpListener::bind(socket).await {
                    Ok(listener) => {
                        let inner_lb = LbServer::init(config).await?;
                        let lb_clone = inner_lb.clone();
                        tokio::spawn({
                            async move {
                                if let Err(e) = lb_clone.listen_for_connections(listener).await {
                                    return Err(LbErrKind::Unexpected(format!(
                                        "Failed to start listening for connections: {e}"
                                    ))
                                    .into());
                                }
                                Ok::<_, LbErr>(())
                            }
                        });
                        Ok(Lb::Direct(inner_lb))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                        Ok(Lb::Network(LbClient { addr: socket, events: EventSubs::default() }))
                    }
                    Err(error) => {
                        Err(LbErrKind::Unexpected(format!("Failed to bind: {error}")).into())
                    }
                }
            }
            None => {
                let inner_lb = LbServer::init(config).await?;
                Ok(Lb::Direct(inner_lb))
            }
        }
    }

    pub async fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        match self {
            Lb::Direct(inner) => inner.create_account(username, api_url, welcome_doc).await,
            Lb::Network(proxy) => proxy.create_account(username, api_url, welcome_doc).await,
        }
    }

    pub async fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        match self {
            Lb::Direct(inner) => inner.import_account(key, api_url).await,
            Lb::Network(proxy) => proxy.import_account(key, api_url).await,
        }
    }

    pub async fn import_account_private_key_v1(&self, account: Account) -> LbResult<Account> {
        match self {
            Lb::Direct(inner) => inner.import_account_private_key_v1(account).await,
            Lb::Network(proxy) => proxy.import_account_private_key_v1(account).await,
        }
    }

    pub async fn export_account_private_key(&self) -> LbResult<String> {
        match self {
            Lb::Direct(inner) => inner.export_account_private_key(),
            Lb::Network(proxy) => proxy.export_account_private_key().await,
        }
    }

    pub(crate) async fn export_account_private_key_v1(&self) -> LbResult<String> {
        match self {
            Lb::Direct(inner) => inner.export_account_private_key_v1(),
            Lb::Network(proxy) => proxy.export_account_private_key_v1().await,
        }
    }

    pub async fn export_account_phrase(&self) -> LbResult<String> {
        match self {
            Lb::Direct(inner) => inner.export_account_phrase(),
            Lb::Network(proxy) => proxy.export_account_phrase().await,
        }
    }

    pub async fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        match self {
            Lb::Direct(inner) => inner.export_account_qr(),
            Lb::Network(proxy) => proxy.export_account_qr().await,
        }
    }

    pub async fn delete_account(&self) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.delete_account().await,
            Lb::Network(proxy) => proxy.delete_account().await,
        }
    }

    pub async fn suggested_docs(&self, settings: RankingWeights) -> LbResult<Vec<Uuid>> {
        match self {
            Lb::Direct(inner) => inner.suggested_docs(settings).await,
            Lb::Network(proxy) => proxy.suggested_docs(settings).await,
        }
    }

    pub async fn disappear_account(&self, username: &str) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.disappear_account(username).await,
            Lb::Network(proxy) => proxy.disappear_account(username).await,
        }
    }

    pub async fn disappear_file(&self, id: Uuid) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.disappear_file(id).await,
            Lb::Network(proxy) => proxy.disappear_file(id).await,
        }
    }

    pub async fn list_users(&self, filter: Option<AccountFilter>) -> LbResult<Vec<Username>> {
        match self {
            Lb::Direct(inner) => inner.list_users(filter).await,
            Lb::Network(proxy) => proxy.list_users(filter).await,
        }
    }

    pub async fn get_account_info(&self, identifier: AccountIdentifier) -> LbResult<AccountInfo> {
        match self {
            Lb::Direct(inner) => inner.get_account_info(identifier).await,
            Lb::Network(proxy) => proxy.get_account_info(identifier).await,
        }
    }

    pub async fn validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
        match self {
            Lb::Direct(inner) => inner.validate_account(username).await,
            Lb::Network(proxy) => proxy.validate_account(username).await,
        }
    }

    pub async fn validate_server(&self) -> LbResult<AdminValidateServer> {
        match self {
            Lb::Direct(inner) => inner.validate_server().await,
            Lb::Network(proxy) => proxy.validate_server().await,
        }
    }

    pub async fn file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse> {
        match self {
            Lb::Direct(inner) => inner.file_info(id).await,
            Lb::Network(proxy) => proxy.file_info(id).await,
        }
    }

    pub async fn rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.rebuild_index(index).await,
            Lb::Network(proxy) => proxy.rebuild_index(index).await,
        }
    }

    pub async fn build_index(&self) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.build_index().await,
            Lb::Network(proxy) => proxy.build_index().await,
        }
    }

    pub async fn set_user_tier(&self, username: &str, info: AdminSetUserTierInfo) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.set_user_tier(username, info).await,
            Lb::Network(proxy) => proxy.set_user_tier(username, info).await,
        }
    }

    pub async fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.upgrade_account_stripe(account_tier).await,
            Lb::Network(proxy) => proxy.upgrade_account_stripe(account_tier).await,
        }
    }

    pub async fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => {
                inner
                    .upgrade_account_google_play(purchase_token, account_id)
                    .await
            }
            Lb::Network(proxy) => {
                proxy
                    .upgrade_account_google_play(purchase_token, account_id)
                    .await
            }
        }
    }

    pub async fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => {
                inner
                    .upgrade_account_app_store(original_transaction_id, app_account_token)
                    .await
            }
            Lb::Network(proxy) => {
                proxy
                    .upgrade_account_app_store(original_transaction_id, app_account_token)
                    .await
            }
        }
    }

    pub async fn cancel_subscription(&self) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.cancel_subscription().await,
            Lb::Network(proxy) => proxy.cancel_subscription().await,
        }
    }

    pub async fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>> {
        match self {
            Lb::Direct(inner) => inner.get_subscription_info().await,
            Lb::Network(proxy) => proxy.get_subscription_info().await,
        }
    }

    pub async fn debug_info(&self, os_info: String) -> LbResult<String> {
        match self {
            Lb::Direct(inner) => inner.debug_info(os_info).await,
            Lb::Network(proxy) => proxy.debug_info(os_info).await,
        }
    }

    pub async fn read_document(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<DecryptedDocument> {
        match self {
            Lb::Direct(inner) => inner.read_document(id, user_activity).await,
            Lb::Network(proxy) => proxy.read_document(id, user_activity).await,
        }
    }

    pub async fn write_document(&self, id: Uuid, content: &[u8]) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.write_document(id, content).await,
            Lb::Network(proxy) => proxy.write_document(id, content).await,
        }
    }

    pub async fn read_document_with_hmac(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        match self {
            Lb::Direct(inner) => inner.read_document_with_hmac(id, user_activity).await,
            Lb::Network(proxy) => proxy.read_document_with_hmac(id, user_activity).await,
        }
    }

    pub async fn safe_write(
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        match self {
            Lb::Direct(inner) => inner.safe_write(id, old_hmac, content).await,
            Lb::Network(proxy) => proxy.safe_write(id, old_hmac, content).await,
        }
    }

    pub async fn subscribe(&self) -> Receiver<Event> {
        match self {
            Lb::Direct(inner) => inner.subscribe(),
            Lb::Network(proxy) => proxy.subscribe().await,
        }
    }

    pub async fn create_file(
        &self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> LbResult<File> {
        match self {
            Lb::Direct(inner) => inner.create_file(name, parent, file_type).await,
            Lb::Network(proxy) => proxy.create_file(name, parent, file_type).await,
        }
    }

    pub async fn rename_file(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.rename_file(id, new_name).await,
            Lb::Network(proxy) => proxy.rename_file(id, new_name).await,
        }
    }

    pub async fn move_file(&self, id: &Uuid, new_parent: &Uuid) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.move_file(id, new_parent).await,
            Lb::Network(proxy) => proxy.move_file(id, new_parent).await,
        }
    }

    pub async fn delete(&self, id: &Uuid) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.delete(id).await,
            Lb::Network(proxy) => proxy.delete(id).await,
        }
    }

    pub async fn root(&self) -> LbResult<File> {
        match self {
            Lb::Direct(inner) => inner.root().await,
            Lb::Network(proxy) => proxy.root().await,
        }
    }

    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        match self {
            Lb::Direct(inner) => inner.list_metadatas().await,
            Lb::Network(proxy) => proxy.list_metadatas().await,
        }
    }

    pub async fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        match self {
            Lb::Direct(inner) => inner.get_children(id).await,
            Lb::Network(proxy) => proxy.get_children(id).await,
        }
    }

    pub async fn get_and_get_children_recursively(&self, id: &Uuid) -> LbResult<Vec<File>> {
        match self {
            Lb::Direct(inner) => inner.get_and_get_children_recursively(id).await,
            Lb::Network(proxy) => proxy.get_and_get_children_recursively(id).await,
        }
    }

    pub async fn get_file_by_id(&self, id: Uuid) -> LbResult<File> {
        match self {
            Lb::Direct(inner) => inner.get_file_by_id(id).await,
            Lb::Network(proxy) => proxy.get_file_by_id(id).await,
        }
    }

    pub async fn local_changes(&self) -> Vec<Uuid> {
        match self {
            Lb::Direct(inner) => inner.local_changes().await,
            Lb::Network(proxy) => proxy.local_changes().await,
        }
    }

    pub async fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &Option<F>,
    ) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.import_files(sources, dest, update_status).await,
            Lb::Network(proxy) => proxy.import_files(sources, dest, update_status).await,
        }
    }

    pub async fn export_file<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, dest: PathBuf, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.export_file(id, dest, edit, update_status).await,
            Lb::Network(proxy) => proxy.export_file(id, dest, edit, update_status).await,
        }
    }

    pub async fn export_file_recursively<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, disk_path: &Path, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => {
                inner
                    .export_file_recursively(id, disk_path, edit, update_status)
                    .await
            }
            Lb::Network(proxy) => {
                proxy
                    .export_file_recursively(id, disk_path, edit, update_status)
                    .await
            }
        }
    }

    pub async fn test_repo_integrity(&self) -> LbResult<Vec<Warning>> {
        match self {
            Lb::Direct(inner) => inner.test_repo_integrity().await,
            Lb::Network(proxy) => proxy.test_repo_integrity().await,
        }
    }

    pub async fn get_account(&self) -> LbResult<Account> {
        match self {
            Lb::Direct(inner) => {
                let acct_ref: &Account = inner.get_account()?;
                Ok(acct_ref.clone())
            }
            Lb::Network(proxy) => proxy.get_account().await,
        }
    }

    pub async fn create_link_at_path(&self, path: &str, target_id: Uuid) -> LbResult<File> {
        match self {
            Lb::Direct(inner) => inner.create_link_at_path(path, target_id).await,
            Lb::Network(proxy) => proxy.create_link_at_path(path, target_id).await,
        }
    }

    pub async fn create_at_path(&self, path: &str) -> LbResult<File> {
        match self {
            Lb::Direct(inner) => inner.create_at_path(path).await,
            Lb::Network(proxy) => proxy.create_at_path(path).await,
        }
    }

    pub async fn get_by_path(&self, path: &str) -> LbResult<File> {
        match self {
            Lb::Direct(inner) => inner.get_by_path(path).await,
            Lb::Network(proxy) => proxy.get_by_path(path).await,
        }
    }

    pub async fn get_path_by_id(&self, id: Uuid) -> LbResult<String> {
        match self {
            Lb::Direct(inner) => inner.get_path_by_id(id).await,
            Lb::Network(proxy) => proxy.get_path_by_id(id).await,
        }
    }

    pub async fn list_paths(&self, filter: Option<Filter>) -> LbResult<Vec<String>> {
        match self {
            Lb::Direct(inner) => inner.list_paths(filter).await,
            Lb::Network(proxy) => proxy.list_paths(filter).await,
        }
    }

    pub async fn list_paths_with_ids(
        &self, filter: Option<Filter>,
    ) -> LbResult<Vec<(Uuid, String)>> {
        match self {
            Lb::Direct(inner) => inner.list_paths_with_ids(filter).await,
            Lb::Network(proxy) => proxy.list_paths_with_ids(filter).await,
        }
    }

    pub async fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => inner.share_file(id, username, mode).await,
            Lb::Network(proxy) => proxy.share_file(id, username, mode).await,
        }
    }

    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>> {
        match self {
            Lb::Direct(inner) => inner.get_pending_shares().await,
            Lb::Network(proxy) => proxy.get_pending_shares().await,
        }
    }

    pub async fn reject_share(&self, id: &Uuid) -> Result<(), LbErr> {
        match self {
            Lb::Direct(inner) => inner.reject_share(id).await,
            Lb::Network(proxy) => proxy.reject_share(id).await,
        }
    }

    pub async fn calculate_work(&self) -> LbResult<SyncStatus> {
        match self {
            Lb::Direct(inner) => inner.calculate_work().await,
            Lb::Network(proxy) => proxy.calculate_work().await,
        }
    }

    pub async fn sync(&self, f: Option<Box<dyn Fn(SyncProgress) + Send>>) -> LbResult<SyncStatus> {
        match self {
            Lb::Direct(inner) => inner.sync(f).await,
            Lb::Network(proxy) => proxy.sync(f).await,
        }
    }

    pub async fn get_last_synced_human(&self) -> LbResult<String> {
        match self {
            Lb::Direct(inner) => inner.get_last_synced_human().await,
            Lb::Network(proxy) => proxy.get_last_synced_human().await,
        }
    }

    pub async fn get_timestamp_human_string(&self, timestamp: i64) -> String {
        match self {
            Lb::Direct(inner) => inner.get_timestamp_human_string(timestamp),
            Lb::Network(proxy) => proxy.get_timestamp_human_string(timestamp).await,
        }
    }

    pub async fn get_usage(&self) -> LbResult<UsageMetrics> {
        match self {
            Lb::Direct(inner) => inner.get_usage().await,
            Lb::Network(proxy) => proxy.get_usage().await,
        }
    }

    pub async fn get_uncompressed_usage_breakdown(&self) -> LbResult<HashMap<Uuid, usize>> {
        match self {
            Lb::Direct(inner) => inner.get_uncompressed_usage_breakdown().await,
            Lb::Network(proxy) => proxy.get_uncompressed_usage_breakdown().await,
        }
    }

    pub async fn get_uncompressed_usage(&self) -> LbResult<UsageItemMetric> {
        match self {
            Lb::Direct(inner) => inner.get_uncompressed_usage().await,
            Lb::Network(proxy) => proxy.get_uncompressed_usage().await,
        }
    }

    pub async fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<Vec<SearchResult>> {
        match self {
            Lb::Direct(inner) => inner.search(input, cfg).await,
            Lb::Network(proxy) => proxy.search(input, cfg).await,
        }
    }

    pub async fn status(&self) -> Status {
        match self {
            Lb::Direct(inner) => inner.status().await,
            Lb::Network(proxy) => proxy.status().await,
        }
    }

    //The follwing methods are not implemented by LbServer but exist in blocking.rs and needed elsewhere
    pub async fn get_config(&self) -> Config {
        match self {
            Lb::Direct(inner) => inner.get_config(),
            Lb::Network(proxy) => proxy.get_config().await,
        }
    }

    pub async fn get_last_synced(&self) -> LbResult<i64> {
        match self {
            Lb::Direct(inner) => inner.get_last_synced().await,
            Lb::Network(proxy) => proxy.get_last_synced().await,
        }
    }

    pub async fn get_search(&self) -> SearchIndex {
        match self {
            Lb::Direct(inner) => inner.get_search(),
            Lb::Network(proxy) => proxy.get_search().await,
        }
    }

    pub async fn get_keychain(&self) -> Keychain {
        match self {
            Lb::Direct(inner) => inner.get_keychain(),
            Lb::Network(proxy) => proxy.get_keychain().await,
        }
    }
}

impl LbServer {
    pub fn get_config(&self) -> Config {
        self.config.clone()
    }
}
