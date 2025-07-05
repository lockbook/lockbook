pub async fn dispatch(lb: Arc<LbServer>, req: RpcRequest) -> LbResult<Vec<u8>> {

    let raw = req.args.unwrap_or_default();
    let payload = match req.method.as_str() {
        "create_account" => {
            let (username, api_url, welcome): (String, String, bool) =
                deserialize_args(&raw)?;
            call_async(|| lb.create_account(&username, &api_url, welcome)).await?
        }

        "import_account" => {
            let (key, maybe_url): (String, Option<String>) = deserialize_args(&raw)?;
            call_async(|| lb.import_account(&key, maybe_url.as_deref())).await?
        }

        "import_account_private_key_v1" => {
            let account: crate::model::account::Account = deserialize_args(&raw)?;
            call_async(|| lb.import_account_private_key_v1(account)).await?
        }

        "import_account_private_key_v2" => {
            let (pk_bytes, api_url): ([u8; 32], String) = deserialize_args(&raw)?;
            let sk = SecretKey::parse(&pk_bytes).map_err(core_err_unexpected)?;
            call_async(|| lb.import_account_private_key_v2(sk, &api_url)).await?
        }

        "import_account_phrase" => {
            let (phrase_vec, api_url): (Vec<String>, String) = deserialize_args(&raw)?;
            let slice: Vec<&str> = phrase_vec.iter().map(|s| s.as_str()).collect();
            let phrase_arr: [&str; 24] = slice
                .try_into()
                .map_err(core_err_unexpected)?;
            call_async(|| lb.import_account_phrase(phrase_arr, &api_url)).await?
        }

        "export_account_private_key" => {
            call_sync(|| lb.export_account_private_key())?
        }

        "export_account_private_key_v1" => {
            call_sync(|| lb.export_account_private_key_v1())?
        }

        "export_account_private_key_v2" => {
            call_sync(|| lb.export_account_private_key_v2())?
        }

        "export_account_phrase" => {
            call_sync(|| lb.export_account_phrase())?
        }

        "export_account_qr" => {
            call_sync(|| lb.export_account_qr())?
        }

        "delete_account" => {
            call_async(|| lb.delete_account()).await?
        }

        "suggested_docs" => {
            let settings: RankingWeights = deserialize_args(&raw)?;
            call_async(|| lb.suggested_docs(settings)).await?
        }

        "disappear_account" => {
            let username: String = deserialize_args(&raw)?;
            call_async(|| lb.disappear_account(&username)).await?
        }

        "disappear_file" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.disappear_file(id)).await?
        }

        "list_users" => {
            let filter: Option<AccountFilter> = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.list_users(filter)).await?
        }

        "get_account_info" => {
            let identifier: AccountIdentifier = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.get_account_info(identifier)).await?
        }

        "validate_account" => {
            let username: String = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.validate_account(&username)).await?
        }

        "validate_server" => {
            call_async(|| lb.validate_server()).await?
        }

        "file_info" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.file_info(id)).await?
        }

        "rebuild_index" => {
            let index: ServerIndex = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.rebuild_index(index)).await?
        }

        "build_index" => {
            call_async(|| lb.build_index()).await?
        }

        "set_user_tier" => {
            let (username, tier_info): (String, AdminSetUserTierInfo) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.set_user_tier(&username, tier_info)).await?
        }

        "upgrade_account_stripe" => {
            let tier: StripeAccountTier = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.upgrade_account_stripe(tier)).await?
        }

        "upgrade_account_google_play" => {
            let (purchase_token, account_id): (String, String) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.upgrade_account_google_play(&purchase_token, &account_id)).await?
        }

        "upgrade_account_app_store" => {
            let (original_transaction_id, app_account_token): (String, String) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.upgrade_account_app_store(original_transaction_id, app_account_token)).await?
        }

        "cancel_subscription" => {
            call_async(|| lb.cancel_subscription()).await?
        }

        "get_subscription_info" => {
            call_async(|| lb.get_subscription_info()).await?
        }

        "debug_info" => {
            let os_info: String = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.debug_info(os_info)).await?
        }

        "read_document" => {
            let (id, user_activity): (Uuid, bool) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.read_document(id, user_activity)).await?
        }

        "write_document" => {
            let (id, content): (Uuid, Vec<u8>) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.write_document(id, &content)).await?
        }

        "read_document_with_hmac" => {
            let (id, user_activity): (Uuid, bool) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.read_document_with_hmac(id, user_activity)).await?
        }

        "safe_write" => {
            let (id, old_hmac, content): (Uuid, Option<DocumentHmac>, Vec<u8>) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.safe_write(id, old_hmac, content)).await?
        }

        "create_file" => {
            let (name, parent, file_type): (String, Uuid, FileType) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.create_file(&name, &parent, file_type)).await?
        }

        "rename_file" => {
            let (id, new_name): (Uuid, String) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.rename_file(&id, &new_name)).await?
        }

        "move_file" => {
            let (id, new_parent): (Uuid, Uuid) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.move_file(&id, &new_parent)).await?
        }

        "delete" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.delete(&id)).await?
        }

        "root" => {
            call_async(|| lb.root()).await?
        }

        "list_metadatas" => {
            call_async(|| lb.list_metadatas()).await?
        }

        "get_children" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.get_children(&id)).await?
        }

        "get_and_get_children_recursively" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.get_and_get_children_recursively(&id)).await?
        }

        "get_file_by_id" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.get_file_by_id(id)).await?
        }

        "local_changes" => {
            let changes: Vec<Uuid> = lb.local_changes().await;
            bincode::serialize(&changes).map_err(core_err_unexpected)?
        }
        //TODO : events module
        "import_files" => {
            let (paths, dest): (Vec<String>, Uuid) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let sources: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
            let statuses = Arc::new(Mutex::new(Vec::new()));

            let cb_statuses = statuses.clone();
            let status_cb = move |status: ImportStatus| {
                let mut guard = cb_statuses.lock().unwrap();
                guard.push(status);
            };
            lb.import_files(&sources, dest, &status_cb).await?;

            let mut buf = Vec::new();
            let mut guard = statuses.lock().unwrap();
            for status in guard.drain(..) {
                let msg: CallbackMessage<ImportStatus, ()> =
                    CallbackMessage::Status(status);
                let bytes = bincode::serialize(&msg).map_err(core_err_unexpected)?;
                buf.extend(&(bytes.len() as u32).to_be_bytes());
                buf.extend(bytes);
            }

            let done_msg: CallbackMessage<ImportStatus, ()> =
                CallbackMessage::Done(Ok(()));
            let done_bytes = bincode::serialize(&done_msg).map_err(core_err_unexpected)?;
            buf.extend(&(done_bytes.len() as u32).to_be_bytes());
            buf.extend(done_bytes);

            buf
        }

        "export_file" => {
            let (id, dest, edit): (Uuid, PathBuf, bool) = deserialize_args(&raw)?;
            call_async(|| lb.export_file(id, dest, edit, &None::<fn(ExportFileInfo)>)).await?
        }

        "export_file_recursively" => {
            let (id, disk_path, edit): (Uuid, PathBuf, bool) = deserialize_args(&raw)?;
            call_async(|| lb.export_file_recursively(id, &disk_path, edit, &None::<fn(ExportFileInfo)>)).await?
        }

        "test_repo_integrity" => {
            call_async(|| lb.test_repo_integrity()).await?
        }

        "get_account" => {
            call_sync(|| lb.get_account()).map_err(core_err_unexpected)?
        }

        "create_link_at_path" => {
            let (path, target_id): (String, Uuid) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.create_link_at_path(&path, target_id)).await?
        }

        "create_at_path" => {
            let path: String = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.create_at_path(&path)).await?
        }

        "get_by_path" => {
            let path: String = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.get_by_path(&path)).await?
        }

        "get_path_by_id" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.get_path_by_id(id)).await?
        }

        "list_paths" => {
            let filter: Option<Filter> = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.list_paths(filter)).await?
        }

        "list_paths_with_ids" => {
            let filter: Option<Filter> = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.list_paths_with_ids(filter)).await?
        }

        "share_file" => {
            let (id, username, mode): (Uuid, String, ShareMode) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.share_file(id, &username, mode)).await?
        }

        "get_pending_shares" => {
            call_async(|| lb.get_pending_shares()).await?
        }

        "reject_share" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.reject_share(&id)).await?
        }

        "calculate_work" => {
            call_async(|| lb.calculate_work()).await?
        }

        "sync" => {
            call_async(|| lb.sync(None)).await?
        }

        "get_last_synced_human" => {
            call_async(|| lb.get_last_synced_human()).await?
        }

        "get_timestamp_human_string" => {
            let timestamp: i64 = deserialize_args(&raw)?;
            call_async(|| async {
                Ok(lb.get_timestamp_human_string(timestamp))
            }).await?
        }
        
        "get_usage" => {
            call_async(|| lb.get_usage()).await?
        }

        "get_uncompressed_usage_breakdown" => {
            call_async(|| lb.get_uncompressed_usage_breakdown()).await?
        }

        "get_uncompressed_usage" => {
            call_async(|| lb.get_uncompressed_usage()).await?
        }

        "search" => {
            let (input, cfg): (String, SearchConfig) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            call_async(|| lb.search(&input, cfg)).await?
        }

        "status" => {
            let res = lb.status().await;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "get_config" => {
            let res = lb.config.clone();
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "get_last_synced" => {
            let tx = lb.ro_tx().await;
            let db = tx.db();
            bincode::serialize(&(db.last_synced.get().copied().unwrap_or(0)))
                .unwrap_or_else(|_| vec![0])
        }

        "get_search" => {
            let res = lb.search.clone();
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "get_keychain" => {
            let res = lb.keychain.clone();
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        other => {
            return Err(LbErrKind::Unexpected(format!("Unknown method: {}", other)).into())
        }
    };

    Ok(payload)
}

fn deserialize_args<A>(raw: &[u8]) -> LbResult<A>
where
    A: DeserializeOwned,
{
    bincode::deserialize(raw)
        .map_err(|e| core_err_unexpected(e).into())
}

async fn call_async<R, Fut>(f: impl FnOnce() -> Fut) -> LbResult<Vec<u8>>
where
    Fut: Future<Output = LbResult<R>>,
    R: Serialize,
{
    let res: R = f().await?;
    bincode::serialize(&res).map_err(|e| core_err_unexpected(e).into())
}

fn call_sync<R>(f: impl FnOnce() -> LbResult<R>) -> LbResult<Vec<u8>>
where
    R: Serialize,
{
    let res: R = f()?;
    bincode::serialize(&res).map_err(|e| core_err_unexpected(e).into())
}


use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use libsecp256k1::SecretKey;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Mutex;
use uuid::Uuid;
use crate::model::api::{AccountFilter, AccountIdentifier, AdminSetUserTierInfo, ServerIndex, StripeAccountTier};
use crate::model::errors::{LbErrKind};
use crate::model::errors::{core_err_unexpected};
use crate::model::file::ShareMode;
use crate::model::file_metadata::{DocumentHmac, FileType};
use crate::model::path_ops::Filter;
use crate::rpc::{CallbackMessage, RpcRequest};
use crate::service::activity::RankingWeights;
use crate::service::import_export::{ExportFileInfo, ImportStatus};
use crate::subscribers::search::SearchConfig;
use crate::{LbServer,LbResult};