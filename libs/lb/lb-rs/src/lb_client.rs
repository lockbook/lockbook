#[derive(Clone)]
pub struct LbClient {
    pub addr: SocketAddrV4,
    pub events: EventSubs
}

impl LbClient {
    pub async fn create_account(
        &self,
        username: &str,
        api_url: &str,
        welcome_doc: bool,
    ) -> LbResult<Account> {
        let args = bincode::serialize(&(username.to_string(), api_url.to_string(), welcome_doc))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "create_account", Some(args)).await
    }

    pub async fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        let args = bincode::serialize(&(key.to_string(),api_url.map(|s| s.to_string()))).map_err(core_err_unexpected)?;
        call_rpc(self.addr, "import_account", Some(args)).await
    }
    
    pub async fn import_account_private_key_v1(&self, account: Account) -> LbResult<Account> {
        let args = bincode::serialize(&account).map_err(core_err_unexpected)?;
        call_rpc(self.addr, "import_account_private_key_v1", Some(args)).await

    }

    pub async fn import_account_private_key_v2(
        &self, private_key: SecretKey, api_url: &str,
    ) -> LbResult<Account> {
        let private_key_bytes=  private_key.serialize();
        let args = bincode::serialize(&(private_key_bytes, api_url.to_string()))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "import_account_private_key_v2", Some(args)).await
    }

    pub async fn import_account_phrase(
        &self, phrase: [&str; 24], api_url: &str,
    ) -> LbResult<Account> {
        let phrase_vec: Vec<String> = phrase.iter().map(|&s| s.to_string()).collect();
        let args = bincode::serialize(&(phrase_vec, api_url.to_string()))
        .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "import_account_phrase", Some(args)).await
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn export_account_private_key(&self) -> LbResult<String> {
        call_rpc(self.addr, "export_account_private_key", None).await
    }

    pub(crate) async  fn export_account_private_key_v1(&self) -> LbResult<String> {
        call_rpc(self.addr, "export_account_private_key_v1", None).await
    }

    pub(crate) async fn export_account_private_key_v2(&self) -> LbResult<String> {
       call_rpc(self.addr, "export_account_private_key_v2", None).await
    }

    pub async fn export_account_phrase(&self) -> LbResult<String> {
        call_rpc(self.addr, "export_account_phrase", None).await
    }

    pub async fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        call_rpc(self.addr, "export_account_qr", None).await
    }

    pub async fn delete_account(&self) -> LbResult<()> {
        call_rpc(self.addr, "delete_account", None).await
    }
    
    pub async fn suggested_docs(&self, settings: RankingWeights) -> LbResult<Vec<Uuid>> {
        let args = bincode::serialize(&settings)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "suggested_docs", Some(args)).await
    }

    pub async fn disappear_account(&self, username: &str) -> LbResult<()>{
        let args = bincode::serialize(&username.to_string())
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "disappear_account", Some(args)).await
    }

    pub async fn disappear_file(&self, id: Uuid) -> LbResult<()>{
        let args = bincode::serialize(&id)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "disappear_file", Some(args)).await
    }

    pub async fn list_users(&self, filter: Option<AccountFilter>) -> LbResult<Vec<Username>>{
        let args = bincode::serialize(&filter)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "list_users", Some(args)).await
    }

    pub async fn get_account_info(&self, identifier: AccountIdentifier) -> LbResult<AccountInfo>{
        let args = bincode::serialize(&identifier)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "get_account_info", Some(args)).await
    }

    pub async fn validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
        let args = bincode::serialize(&username.to_string())
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "validate_account", Some(args)).await
    }

    pub async fn validate_server(&self) -> LbResult<AdminValidateServer>{
        call_rpc(self.addr, "validate_server",None).await
    }

    pub async fn file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse>{
        let args = bincode::serialize(&id)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "file_info", Some(args)).await
    }

    pub async fn rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        let args = bincode::serialize(&index)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "rebuild_index", Some(args)).await
    }

    pub async fn build_index(&self) -> LbResult<()>{
        call_rpc(self.addr, "build_index", None).await
    }

    pub async fn set_user_tier(&self, username: &str, info: AdminSetUserTierInfo) -> LbResult<()> {
        let args = bincode::serialize(&(username.to_string(),info))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "set_user_tier", Some(args)).await
    }

    pub async fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()>{
        let args = bincode::serialize(&account_tier)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "upgrade_account_stripe", Some(args)).await
    }

    pub async fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()>{
        let args = bincode::serialize(&(purchase_token.to_string(),account_id.to_string()))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "upgrade_account_google_play", Some(args)).await
    }

    pub async fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()>{
        let args = bincode::serialize(&(original_transaction_id,app_account_token))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "upgrade_account_app_store", Some(args)).await
    }

    pub async fn cancel_subscription(&self) -> LbResult<()>{
        call_rpc(self.addr, "cancel_subscription", None).await
    }

    pub async fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>>{
        call_rpc(self.addr, "get_subscription_info", None).await
    }

    pub async fn debug_info(&self, os_info: String) -> LbResult<String>{
        let args = bincode::serialize(&os_info)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "debug_info", Some(args)).await
    }

    pub async fn read_document(
        &self, id: Uuid, user_activity: bool,
    ) -> LbResult<DecryptedDocument>{
        let args = bincode::serialize(&(id,user_activity))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "read_document", Some(args)).await
    }

    pub async fn write_document(
        &self,
        id: Uuid,
        content: &[u8],
    ) -> LbResult<()> {
        let args = bincode::serialize(&(id, content))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "write_document", Some(args)).await
    }

    pub async fn read_document_with_hmac(
        &self,
        id: Uuid,
        user_activity: bool,
    ) -> LbResult<(Option<DocumentHmac>, DecryptedDocument)> {
        let args = bincode::serialize(&(id, user_activity))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr,"read_document_with_hmac",Some(args)).await
    }

    pub async fn safe_write(
        &self,
        id: Uuid,
        old_hmac: Option<DocumentHmac>,
        content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        let args = bincode::serialize(&(id, old_hmac, content))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "safe_write", Some(args)).await
    }

    pub async fn subscribe(&self) -> Receiver<Event> {
       //todo!("implement subscribe for proxylb");
       self.events.get_tx().subscribe()
    }

    pub async fn create_file(
        &self,
        name: &str,
        parent: &Uuid,
        file_type: FileType,
    ) -> LbResult<File> {
        let args = bincode::serialize(&(name.to_string(), *parent, file_type))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "create_file", Some(args)).await
    }

    pub async fn rename_file(
        &self,
        id: &Uuid,
        new_name: &str,
    ) -> LbResult<()> {
        let args = bincode::serialize(&(*id, new_name.to_string()))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "rename_file", Some(args)).await
    }

    pub async fn move_file(
        &self,
        id: &Uuid,
        new_parent: &Uuid,
    ) -> LbResult<()> {
        let args = bincode::serialize(&(*id, *new_parent))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "move_file", Some(args)).await
    }

    pub async fn delete(
        &self,
        id: &Uuid,
    ) -> LbResult<()> {
        let args = bincode::serialize(id)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "delete", Some(args)).await
    }

    pub async fn root(&self) -> LbResult<File> {
        call_rpc(self.addr, "root", None).await
    }

    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        call_rpc(self.addr, "list_metadatas", None).await
    }

    pub async fn get_children(
        &self,
        id: &Uuid,
    ) -> LbResult<Vec<File>> {
        let args = bincode::serialize(id)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "get_children", Some(args)).await
    }

    pub async fn get_and_get_children_recursively(
        &self,
        id: &Uuid,
    ) -> LbResult<Vec<File>> {
        let args = bincode::serialize(id)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "get_and_get_children_recursively", Some(args)).await
    }

    pub async fn get_file_by_id(
        &self,
        id: Uuid,
    ) -> LbResult<File> {
        let args = bincode::serialize(&id)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "get_file_by_id", Some(args)).await
    }

    pub async fn local_changes(&self) -> Vec<Uuid> {
        call_rpc(self.addr, "local_changes", None)
            .await
            .unwrap()         
    }

    pub async fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> LbResult<()>{
        let source_paths: Vec<String> = sources.iter().map(|path| path.to_string_lossy().into_owned()).collect();
        let args = bincode::serialize(&(source_paths, dest))
            .map_err(core_err_unexpected)?;
        call_rpc_with_callback::<ImportStatus, (), _>(
            self.addr,
            "import_files",
            Some(args),
            update_status,
        )
        .await
    }

    pub async fn export_file<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, dest: PathBuf, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()>{
        let args = bincode::serialize(&(id,dest.clone(),edit))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "export_file", Some(args)).await
    }

    pub async fn export_file_recursively<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, disk_path: &Path, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()>{
        let args = bincode::serialize(&(id,disk_path.to_path_buf(),edit))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "export_file_recursively", Some(args)).await
    }

    pub async fn test_repo_integrity(&self) -> LbResult<Vec<Warning>>{
        call_rpc(self.addr, "test_repo_integrity", None).await
    }

    pub async fn get_account(&self) -> LbResult<Account>{
        call_rpc(self.addr, "get_account", None).await
    }

    pub async fn create_link_at_path(&self, path: &str, target_id: Uuid) -> LbResult<File>{
        let args = bincode::serialize(&(path.to_string(), target_id))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "create_link_at_path", Some(args)).await
    }

    pub async fn create_at_path(&self, path: &str) -> LbResult<File>{
        let args = bincode::serialize(&path.to_string())
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "create_at_path", Some(args)).await
    }

    pub async fn get_by_path(&self, path: &str) -> LbResult<File>{
        let args = bincode::serialize(&path.to_string())
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "get_by_path", Some(args)).await
    }

    pub async fn get_path_by_id(&self, id: Uuid) -> LbResult<String>{
        let args = bincode::serialize(&id)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "get_path_by_id", Some(args)).await
    }

    pub async fn list_paths(&self, filter: Option<Filter>) -> LbResult<Vec<String>>{
        let args = bincode::serialize(&filter)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "list_paths", Some(args)).await
    }

    pub async fn list_paths_with_ids(
        &self, filter: Option<Filter>,
    ) -> LbResult<Vec<(Uuid, String)>>{
        let args = bincode::serialize(&filter)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "list_paths_with_ids", Some(args)).await
    }

    pub async fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()>{
        let args = bincode::serialize(&(id,username.to_string(),mode))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "share_file", Some(args)).await
    }

    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>>{
        call_rpc(self.addr, "get_pending_shares", None).await
    }

    pub async fn reject_share(&self, id: &Uuid) -> Result<(), LbErr>{
        let args = bincode::serialize(&id)
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "reject_share", Some(args)).await
    }

    pub async fn calculate_work(&self) -> LbResult<SyncStatus>{
        call_rpc(self.addr, "calculate_work", None).await
    }

    pub async fn sync(&self, f: Option<Box<dyn Fn(SyncProgress) + Send>>) -> LbResult<SyncStatus>{
        call_rpc(self.addr, "sync", None).await
    }

    pub async fn get_last_synced_human(&self) -> LbResult<String>{
        call_rpc(self.addr, "get_last_synced_human", None).await
    }

    pub async fn get_timestamp_human_string(&self, timestamp: i64) -> String{
        let args = bincode::serialize(&timestamp).map_err(core_err_unexpected).unwrap();
        call_rpc(self.addr, "sync", Some(args)).await.unwrap()
    }

    pub async fn get_usage(&self) -> LbResult<UsageMetrics>{
        call_rpc(self.addr, "get_usage", None).await
    }

    pub async fn get_uncompressed_usage_breakdown(&self) -> LbResult<HashMap<Uuid, usize>>{
        call_rpc(self.addr, "get_uncompressed_usage_breakdown", None).await
    }

    pub async fn get_uncompressed_usage(&self) -> LbResult<UsageItemMetric>{
        call_rpc(self.addr, "get_uncompressed_usage", None).await
    }

    pub async fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<Vec<SearchResult>>{
        let args = bincode::serialize(&(input.to_string(),cfg))
            .map_err(core_err_unexpected)?;
        call_rpc(self.addr, "search", Some(args)).await
    }

    pub async fn status(&self) -> Status{
        call_rpc(self.addr, "status", None).await.unwrap()
    }

    pub async fn get_config(&self) -> Config {
        call_rpc(self.addr, "get_config", None).await.unwrap()
    }

    pub async fn get_last_synced(&self) -> LbResult<i64> {
        call_rpc(self.addr, "get_last_synced", None).await
    }

    pub async fn get_search(&self) -> SearchIndex {
        call_rpc(self.addr, "get_search", None).await.unwrap()
    }

    pub async fn get_keychain(&self) -> Keychain {
        call_rpc(self.addr, "get_keychain", None).await.unwrap()
    }
}

use crate::model::core_config::Config;
use crate::service::events::EventSubs;
use crate::service::keychain::Keychain;
use crate::service::sync::{SyncProgress, SyncStatus};
use crate::subscribers::search::{SearchConfig, SearchIndex, SearchResult};
use crate::subscribers::status::Status;
use crate::{model::errors::core_err_unexpected};
use libsecp256k1::SecretKey;
use crate::rpc::{call_rpc, call_rpc_with_callback};
use crate::Uuid;
use std::collections::HashMap;
use std::net::SocketAddrV4;
use std::path::{Path, PathBuf};
use tokio::sync::broadcast::Receiver;
use crate::model::{account::{Account, Username}, api::{AccountFilter, AccountIdentifier, AccountInfo, AdminFileInfoResponse, AdminSetUserTierInfo, AdminValidateAccount, AdminValidateServer, ServerIndex, StripeAccountTier, SubscriptionInfo}, crypto::DecryptedDocument, file::{File, ShareMode}, file_metadata::{DocumentHmac,FileType}, errors::{Warning, LbErr}, path_ops::Filter};
use crate::service::{activity::RankingWeights, events::Event, import_export::{ExportFileInfo, ImportStatus}, usage::{UsageItemMetric, UsageMetrics}};
use crate::LbResult;