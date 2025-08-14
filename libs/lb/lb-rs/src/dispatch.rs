use crate::model::api::{
    AccountFilter, AccountIdentifier, AdminSetUserTierInfo, ServerIndex, StripeAccountTier,
};
use crate::model::errors::core_err_unexpected;
use crate::model::errors::LbErrKind;
use crate::model::file::ShareMode;
use crate::model::file_metadata::{DocumentHmac, FileType};
use crate::model::path_ops::Filter;
use crate::rpc::Method;
use crate::service::activity::RankingWeights;
use crate::service::import_export::{ExportFileInfo, ImportStatus};
use crate::subscribers::search::SearchConfig;
use crate::{LbResult, LbServer};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

pub async fn dispatch(lb: Arc<LbServer>, method: Method, raw: &[u8]) -> LbResult<Vec<u8>> {
    let res = match method {
        Method::CreateAccount => {
            let (username, api_url, welcome): (String, String, bool) = deserialize_args(raw)?;
            call_async(|| lb.create_account(&username, &api_url, welcome)).await?
        }

        Method::ImportAccount => {
            let (key, maybe_url): (String, Option<String>) = deserialize_args(raw)?;
            call_async(|| lb.import_account(&key, maybe_url.as_deref())).await?
        }

        Method::ImportAccountPrivateKeyV1 => {
            let account: crate::model::account::Account = deserialize_args(raw)?;
            call_async(|| lb.import_account_private_key_v1(account)).await?
        }

        Method::ExportAccountPrivateKey => call_sync(|| lb.export_account_private_key())?,

        Method::ExportAccountPrivateKeyV1 => call_sync(|| lb.export_account_private_key_v1())?,

        Method::ExportAccountPrivateKeyV2 => call_sync(|| lb.export_account_private_key_v2())?,

        Method::ExportAccountPhrase => call_sync(|| lb.export_account_phrase())?,

        Method::ExportAccountQr => call_sync(|| lb.export_account_qr())?,

        Method::DeleteAccount => call_async(|| lb.delete_account()).await?,

        Method::SuggestedDocs => {
            let settings: RankingWeights = deserialize_args(raw)?;
            call_async(|| lb.suggested_docs(settings)).await?
        }

        Method::DisappearAccount => {
            let username: String = deserialize_args(raw)?;
            call_async(|| lb.disappear_account(&username)).await?
        }

        Method::DisappearFile => {
            let id: Uuid = deserialize_args(raw)?;
            call_async(|| lb.disappear_file(id)).await?
        }

        Method::ListUsers => {
            let filter: Option<AccountFilter> = deserialize_args(raw)?;
            call_async(|| lb.list_users(filter)).await?
        }

        Method::GetAccountInfo => {
            let identifier: AccountIdentifier = deserialize_args(raw)?;
            call_async(|| lb.get_account_info(identifier)).await?
        }

        Method::ValidateAccount => {
            let username: String = deserialize_args(raw)?;
            call_async(|| lb.validate_account(&username)).await?
        }

        Method::ValidateServer => call_async(|| lb.validate_server()).await?,

        Method::FileInfo => {
            let id: Uuid = deserialize_args(raw)?;
            call_async(|| lb.file_info(id)).await?
        }

        Method::RebuildIndex => {
            let index: ServerIndex = deserialize_args(raw)?;
            call_async(|| lb.rebuild_index(index)).await?
        }

        Method::BuildIndex => call_async(|| lb.build_index()).await?,

        Method::SetUserTier => {
            let (username, tier_info): (String, AdminSetUserTierInfo) = deserialize_args(raw)?;
            call_async(|| lb.set_user_tier(&username, tier_info)).await?
        }

        Method::UpgradeAccountStripe => {
            let tier: StripeAccountTier = deserialize_args(raw)?;
            call_async(|| lb.upgrade_account_stripe(tier)).await?
        }

        Method::UpgradeAccountGooglePlay => {
            let (purchase_token, account_id): (String, String) = deserialize_args(raw)?;
            call_async(|| lb.upgrade_account_google_play(&purchase_token, &account_id)).await?
        }

        Method::UpgradeAccountAppStore => {
            let (original_transaction_id, app_account_token): (String, String) =
                deserialize_args(raw)?;
            call_async(|| lb.upgrade_account_app_store(original_transaction_id, app_account_token))
                .await?
        }

        Method::CancelSubscription => call_async(|| lb.cancel_subscription()).await?,

        Method::GetSubscriptionInfo => call_async(|| lb.get_subscription_info()).await?,

        Method::DebugInfo => {
            let os_info: String = deserialize_args(raw)?;
            call_async(|| lb.debug_info(os_info)).await?
        }

        Method::ReadDocument => {
            let (id, user_activity): (Uuid, bool) = deserialize_args(raw)?;
            call_async(|| lb.read_document(id, user_activity)).await?
        }

        Method::WriteDocument => {
            let (id, content): (Uuid, Vec<u8>) = deserialize_args(raw)?;
            call_async(|| lb.write_document(id, &content)).await?
        }

        Method::ReadDocumentWithHmac => {
            let (id, user_activity): (Uuid, bool) = deserialize_args(raw)?;
            call_async(|| lb.read_document_with_hmac(id, user_activity)).await?
        }

        Method::SafeWrite => {
            let (id, old_hmac, content): (Uuid, Option<DocumentHmac>, Vec<u8>) =
                deserialize_args(raw)?;
            call_async(|| lb.safe_write(id, old_hmac, content)).await?
        }

        Method::CreateFile => {
            let (name, parent, file_type): (String, Uuid, FileType) = deserialize_args(raw)?;
            call_async(|| lb.create_file(&name, &parent, file_type)).await?
        }

        Method::RenameFile => {
            let (id, new_name): (Uuid, String) = deserialize_args(raw)?;
            call_async(|| lb.rename_file(&id, &new_name)).await?
        }

        Method::MoveFile => {
            let (id, new_parent): (Uuid, Uuid) = deserialize_args(raw)?;
            call_async(|| lb.move_file(&id, &new_parent)).await?
        }

        Method::Delete => {
            let id: Uuid = deserialize_args(raw)?;
            call_async(|| lb.delete(&id)).await?
        }

        Method::Root => call_async(|| lb.root()).await?,

        Method::ListMetadatas => call_async(|| lb.list_metadatas()).await?,

        Method::GetChildren => {
            let id: Uuid = deserialize_args(raw)?;
            call_async(|| lb.get_children(&id)).await?
        }

        Method::GetAndGetChildrenRecursively => {
            let id: Uuid = deserialize_args(raw)?;
            call_async(|| lb.get_and_get_children_recursively(&id)).await?
        }

        Method::GetFileById => {
            let id: Uuid = deserialize_args(raw)?;
            call_async(|| lb.get_file_by_id(id)).await?
        }

        Method::LocalChanges => {
            call_async(|| async move {
                let changes: Vec<Uuid> = lb.local_changes().await;
                Ok(changes)
            })
            .await?
        }

        Method::ImportFiles => {
            let (paths, dest): (Vec<String>, Uuid) =
                bincode::deserialize(raw).map_err(core_err_unexpected)?;
            let sources: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
            call_async(|| lb.import_files(&sources, dest, &None::<fn(ImportStatus)>)).await?
        }

        Method::ExportFile => {
            let (id, dest, edit): (Uuid, PathBuf, bool) = deserialize_args(raw)?;
            call_async(|| lb.export_file(id, dest, edit, &None::<fn(ExportFileInfo)>)).await?
        }

        Method::ExportFileRecursively => {
            let (id, disk_path, edit): (Uuid, PathBuf, bool) = deserialize_args(raw)?;
            call_async(|| {
                lb.export_file_recursively(id, &disk_path, edit, &None::<fn(ExportFileInfo)>)
            })
            .await?
        }

        Method::TestRepoIntegrity => call_async(|| lb.test_repo_integrity()).await?,

        Method::GetAccount => call_sync(|| lb.get_account()).map_err(core_err_unexpected)?,

        Method::CreateLinkAtPath => {
            let (path, target_id): (String, Uuid) = deserialize_args(raw)?;
            call_async(|| lb.create_link_at_path(&path, target_id)).await?
        }

        Method::CreateAtPath => {
            let path: String = deserialize_args(raw)?;
            call_async(|| lb.create_at_path(&path)).await?
        }

        Method::GetByPath => {
            let path: String = deserialize_args(raw)?;
            call_async(|| lb.get_by_path(&path)).await?
        }

        Method::GetPathById => {
            let id: Uuid = deserialize_args(raw)?;
            call_async(|| lb.get_path_by_id(id)).await?
        }

        Method::ListPaths => {
            let filter: Option<Filter> = deserialize_args(raw)?;
            call_async(|| lb.list_paths(filter)).await?
        }

        Method::ListPathsWithIds => {
            let filter: Option<Filter> = deserialize_args(raw)?;
            call_async(|| lb.list_paths_with_ids(filter)).await?
        }

        Method::ShareFile => {
            let (id, username, mode): (Uuid, String, ShareMode) = deserialize_args(raw)?;
            call_async(|| lb.share_file(id, &username, mode)).await?
        }

        Method::GetPendingShares => call_async(|| lb.get_pending_shares()).await?,

        Method::RejectShare => {
            let id: Uuid = deserialize_args(raw)?;
            call_async(|| lb.reject_share(&id)).await?
        }

        Method::CalculateWork => call_async(|| lb.calculate_work()).await?,

        Method::Sync => call_async(|| lb.sync(None)).await?,

        Method::GetLastSyncedHuman => call_async(|| lb.get_last_synced_human()).await?,

        Method::GetTimestampHumanString => {
            let timestamp: i64 = deserialize_args(raw)?;
            call_async(|| async { Ok(lb.get_timestamp_human_string(timestamp)) }).await?
        }

        Method::GetUsage => call_async(|| lb.get_usage()).await?,

        Method::GetUncompressedUsageBreakdown => {
            call_async(|| lb.get_uncompressed_usage_breakdown()).await?
        }

        Method::GetUncompressedUsage => call_async(|| lb.get_uncompressed_usage()).await?,

        Method::Search => {
            let (input, cfg): (String, SearchConfig) = deserialize_args(raw)?;
            call_async(|| lb.search(&input, cfg)).await?
        }

        Method::Status => {
            call_async(|| async {
                let status = lb.status().await;
                Ok(status)
            })
            .await?
        }

        Method::GetConfig => call_async(|| async { Ok(lb.get_config()) }).await?,

        Method::GetLastSynced => call_async(|| async { Ok(lb.get_last_synced().await) }).await?,

        Method::GetSearch => call_async(|| async { Ok(lb.get_search()) }).await?,

        Method::GetKeychain => call_async(|| async { Ok(lb.get_keychain()) }).await?,

        other => return Err(LbErrKind::Unexpected(format!("Unknown method: {:?}", other)).into()),
    };
    Ok(res)
}

fn deserialize_args<A>(raw: &[u8]) -> LbResult<A>
where
    A: DeserializeOwned,
{
    bincode::deserialize(raw).map_err(|e| core_err_unexpected(e).into())
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
