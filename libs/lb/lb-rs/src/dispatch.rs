pub async fn dispatch(lb: Arc<LbServer>, req: RpcRequest) -> LbResult<Vec<u8>> {

    let raw = req.args.unwrap_or_default();
    let payload = match req.method.as_str() {
        "create_account" => {
            let (username, api_url, welcome): (String, String, bool) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.create_account(&username, &api_url, welcome).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account" => {
            let (key, maybe_url): (String, Option<String>) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.import_account(&key, maybe_url.as_deref()).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account_private_key_v1" => {
            let account: crate::model::account::Account =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.import_account_private_key_v1(account).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account_private_key_v2" => {
            let (pk_bytes, api_url): ( [u8; 32], String ) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let sk = SecretKey::parse(&pk_bytes)
                .map_err(core_err_unexpected)?;
            let res = lb.import_account_private_key_v2(sk, &api_url).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account_phrase" => {
            let (phrase_vec, api_url): (Vec<String>, String) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let slice: Vec<&str> = phrase_vec.iter().map(|s| s.as_str()).collect();
            let phrase_arr: [&str; 24] = slice
                .try_into()
                .map_err(core_err_unexpected)?;
            
            let res = lb.import_account_phrase(phrase_arr, &api_url).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_private_key" => {
            let res: String = lb.export_account_private_key()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_private_key_v1" => {
            let res: String = lb.export_account_private_key_v1()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_private_key_v2" => {
            let res: String = lb.export_account_private_key_v2()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_phrase" => {
            let res: String = lb.export_account_phrase()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_qr" => {
            let res: Vec<u8> = lb.export_account_qr()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "delete_account" => {
            lb.delete_account().await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "disappear_account" => {
            let username: String =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.disappear_account(&username).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "disappear_file" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.disappear_file(id).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "list_users" => {
            let filter: Option<AccountFilter> =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let users = lb.list_users(filter).await?;
            bincode::serialize(&users).map_err(core_err_unexpected)?
        }

        "get_account_info" => {
            let identifier: AccountIdentifier =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let info = lb.get_account_info(identifier).await?;
            bincode::serialize(&info).map_err(core_err_unexpected)?
        }

        "validate_account" => {
            let username: String =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let validation = lb.validate_account(&username).await?;
            bincode::serialize(&validation).map_err(core_err_unexpected)?
        }

        "validate_server" => {
            let server_info = lb.validate_server().await?;
            bincode::serialize(&server_info).map_err(core_err_unexpected)?
        }

        "file_info" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let info = lb.file_info(id).await?;
            bincode::serialize(&info).map_err(core_err_unexpected)?
        }

        "rebuild_index" => {
            let index: ServerIndex =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.rebuild_index(index).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "set_user_tier" => {
            let (username, tier_info): (String, AdminSetUserTierInfo) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.set_user_tier(&username, tier_info).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "upgrade_account_stripe" => {
            let tier: StripeAccountTier =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.upgrade_account_stripe(tier).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "upgrade_account_google_play" => {
            let (purchase_token, account_id): (String, String) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.upgrade_account_google_play(&purchase_token, &account_id).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "upgrade_account_app_store" => {
            let (original_transaction_id, app_account_token): (String, String) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.upgrade_account_app_store(original_transaction_id, app_account_token)
                .await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "cancel_subscription" => {
            lb.cancel_subscription().await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "get_subscription_info" => {
            let info: Option<SubscriptionInfo> = lb.get_subscription_info().await?;
            bincode::serialize(&info).map_err(core_err_unexpected)?
        }

        "debug_info" => {
            let os_info: String = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let debug_str = lb.debug_info(os_info).await?;
            bincode::serialize(&debug_str).map_err(core_err_unexpected)?
        }

        "read_document" => {
            let (id, user_activity): (Uuid, bool) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let doc = lb.read_document(id, user_activity).await?;
            bincode::serialize(&doc).map_err(core_err_unexpected)?
        }

        "write_document" => {
            let (id, content): (Uuid, Vec<u8>) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.write_document(id, &content).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "read_document_with_hmac" => {
            let (id, user_activity): (Uuid, bool) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let pair: (Option<DocumentHmac>, DecryptedDocument) =
                lb.read_document_with_hmac(id, user_activity).await?;
            bincode::serialize(&pair).map_err(core_err_unexpected)?
        }

        "safe_write" => {
            let (id, old_hmac, content): (Uuid, Option<DocumentHmac>, Vec<u8>) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let new_hmac = lb.safe_write(id, old_hmac, content).await?;
            bincode::serialize(&new_hmac).map_err(core_err_unexpected)?
        }

        "create_file" => {
            let (name, parent, file_type): (String, Uuid, FileType) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let file = lb.create_file(&name, &parent, file_type).await?;
            bincode::serialize(&file).map_err(core_err_unexpected)?
        }

        "rename_file" => {
            let (id, new_name): (Uuid, String) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.rename_file(&id, &new_name).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "move_file" => {
            let (id, new_parent): (Uuid, Uuid) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.move_file(&id, &new_parent).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "delete" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.delete(&id).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "root" => {
            let file = lb.root().await?;
            bincode::serialize(&file).map_err(core_err_unexpected)?
        }

        "list_metadatas" => {
            let files = lb.list_metadatas().await?;
            bincode::serialize(&files).map_err(core_err_unexpected)?
        }

        "get_children" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let children = lb.get_children(&id).await?;
            bincode::serialize(&children).map_err(core_err_unexpected)?
        }

        "get_and_get_children_recursively" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let all = lb.get_and_get_children_recursively(&id).await?;
            bincode::serialize(&all).map_err(core_err_unexpected)?
        }

        "get_file_by_id" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let file = lb.get_file_by_id(id).await?;
            bincode::serialize(&file).map_err(core_err_unexpected)?
        }

        "local_changes" => {
            let changes: Vec<Uuid> = lb.local_changes().await;
            bincode::serialize(&changes).map_err(core_err_unexpected)?
        }
        //TODO : events module and import_export module
        "test_repo_integrity" => {
            let changes: Vec<Warning> = lb.test_repo_integrity().await?;
            bincode::serialize(&changes).map_err(core_err_unexpected)?
        }

        "get_account" => {
            let account_ref: &Account = lb.get_account()?;
            bincode::serialize(account_ref).map_err(core_err_unexpected)?
        }

        "create_link_at_path" => {
            let (path, target_id): (String, Uuid) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.create_link_at_path(&path, target_id).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "create_at_path" => {
            let path: String = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.create_at_path(&path).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "get_by_path" => {
            let path: String = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.get_by_path(&path).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "get_path_by_id" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.get_path_by_id(id).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "list_paths" => {
            let filter: Option<Filter> = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.list_paths(filter).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "list_paths_with_ids" => {
            let filter: Option<Filter> = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.list_paths_with_ids(filter).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "share_file" => {
            let (id, username, mode): (Uuid, String, ShareMode) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.share_file(id, &username, mode).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        "get_pending_shares" => {
            let res = lb.get_pending_shares().await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "reject_share" => {
            let id: Uuid = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            lb.reject_share(&id).await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }
        //TODO sync api
        other => {
            return Err(LbErrKind::Unexpected(format!("Unknown method: {}", other)).into())
        }
    };

    Ok(payload)
}

use std::sync::Arc;

use libsecp256k1::SecretKey;
use uuid::Uuid;
use crate::model::account::Account;
use crate::model::api::{AccountFilter, AccountIdentifier, AdminSetUserTierInfo, ServerIndex, StripeAccountTier, SubscriptionInfo};
use crate::model::crypto::DecryptedDocument;
use crate::model::errors::{LbErrKind, Warning};
use crate::model::errors::{core_err_unexpected};
use crate::model::file::ShareMode;
use crate::model::file_metadata::{DocumentHmac, FileType};
use crate::model::path_ops::Filter;
use crate::rpc::RpcRequest;
use crate::{LbServer,LbResult};