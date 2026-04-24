use std::io;
use std::sync::Arc;

use serde::Serialize;
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio::sync::broadcast::error::RecvError;

use crate::LocalLb;
use crate::ipc::protocol::{Frame, Request};
use crate::model::errors::LbResult;

pub async fn serve(listener: UnixListener, lb: Arc<LocalLb>) {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let lb = Arc::clone(&lb);
                tokio::spawn(async move {
                    if let Err(err) = handle_conn(stream, lb).await {
                        if err.kind() == io::ErrorKind::UnexpectedEof {
                            tracing::debug!("ipc guest disconnected");
                        } else {
                            tracing::warn!(?err, "ipc connection ended");
                        }
                    }
                });
            }
            Err(err) => {
                tracing::error!(?err, "ipc accept failed; aborting serve loop");
                return;
            }
        }
    }
}

async fn handle_conn(stream: UnixStream, lb: Arc<LocalLb>) -> io::Result<()> {
    let (mut reader, write_half) = stream.into_split();
    let writer = Arc::new(Mutex::new(write_half));

    loop {
        let frame = Frame::read(&mut reader).await?;
        match frame {
            Frame::Request { seq, body: Request::Subscribe } => {
                let lb_for_task = Arc::clone(&lb);
                let writer_for_task = Arc::clone(&writer);
                tokio::spawn(forward_events(lb_for_task, writer_for_task, seq));
                send_response(&writer, seq, enc_plain(())).await?;
            }
            Frame::Request { seq, body } => {
                let output = dispatch(&lb, body).await;
                send_response(&writer, seq, output).await?;
            }
            Frame::Response { .. } | Frame::Event { .. } | Frame::EventEnd { .. } => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "guest sent a host-only frame",
                ));
            }
        }
    }
}

async fn send_response(
    writer: &Arc<Mutex<OwnedWriteHalf>>, seq: u64, output: Vec<u8>,
) -> io::Result<()> {
    let response = Frame::Response { seq, output };
    let mut w = writer.lock().await;
    response.write(&mut *w).await
}

async fn forward_events(lb: Arc<LocalLb>, writer: Arc<Mutex<OwnedWriteHalf>>, stream_seq: u64) {
    let mut rx = lb.subscribe();
    loop {
        match rx.recv().await {
            Ok(event) => {
                let frame = Frame::Event { stream_seq, body: event };
                let mut w = writer.lock().await;
                if let Err(err) = frame.write(&mut *w).await {
                    tracing::debug!(?err, "ipc: event forward failed");
                    return;
                }
            }
            Err(RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "ipc: event subscriber lagged");
                continue;
            }
            Err(RecvError::Closed) => break,
        }
    }

    let frame = Frame::EventEnd { stream_seq };
    let mut w = writer.lock().await;
    let _ = frame.write(&mut *w).await;
}

fn enc<Out: Serialize>(result: LbResult<Out>) -> Vec<u8> {
    bincode::serialize(&result).unwrap_or_else(|e| {
        tracing::error!(?e, "ipc: serialize response failed");
        Vec::new()
    })
}

fn enc_plain<Out: Serialize>(value: Out) -> Vec<u8> {
    enc::<Out>(Ok(value))
}

async fn dispatch(lb: &LocalLb, req: Request) -> Vec<u8> {
    match req {
        Request::CreateAccount { username, api_url, welcome_doc } => {
            enc(lb.create_account(&username, &api_url, welcome_doc).await)
        }
        Request::ImportAccount { key, api_url } => {
            enc(lb.import_account(&key, api_url.as_deref()).await)
        }
        Request::ImportAccountPrivateKeyV1 { account } => {
            enc(lb.import_account_private_key_v1(account).await)
        }
        Request::ImportAccountPhrase { phrase, api_url } => {
            match <[String; 24]>::try_from(phrase) {
                Ok(owned) => {
                    let refs: [&str; 24] = std::array::from_fn(|i| owned[i].as_str());
                    enc(lb.import_account_phrase(refs, &api_url).await)
                }
                Err(v) => enc::<crate::model::account::Account>(Err(
                    crate::model::errors::LbErrKind::Unexpected(format!(
                        "ipc: import_account_phrase expected 24 words, got {}",
                        v.len()
                    ))
                    .into(),
                )),
            }
        }
        Request::DeleteAccount => enc(lb.delete_account().await),
        Request::GetAccount => enc(lb.get_account().cloned()),

        Request::SuggestedDocs { settings } => enc(lb.suggested_docs(settings).await),
        Request::ClearSuggested => enc(lb.clear_suggested().await),
        Request::ClearSuggestedId { id } => enc(lb.clear_suggested_id(id).await),
        Request::AppForegrounded => {
            lb.app_foregrounded();
            enc_plain(())
        }

        Request::DisappearAccount { username } => enc(lb.disappear_account(&username).await),
        Request::DisappearFile { id } => enc(lb.disappear_file(id).await),
        Request::ListUsers { filter } => enc(lb.list_users(filter).await),
        Request::GetAccountInfo { identifier } => enc(lb.get_account_info(identifier).await),
        Request::AdminValidateAccount { username } => enc(lb.validate_account(&username).await),
        Request::AdminValidateServer => enc(lb.validate_server().await),
        Request::AdminFileInfo { id } => enc(lb.file_info(id).await),
        Request::RebuildIndex { index } => enc(lb.rebuild_index(index).await),
        Request::SetUserTier { username, info } => enc(lb.set_user_tier(&username, info).await),

        Request::UpgradeAccountStripe { account_tier } => {
            enc(lb.upgrade_account_stripe(account_tier).await)
        }
        Request::UpgradeAccountGooglePlay { purchase_token, account_id } => enc(lb
            .upgrade_account_google_play(&purchase_token, &account_id)
            .await),
        Request::UpgradeAccountAppStore { original_transaction_id, app_account_token } => enc(lb
            .upgrade_account_app_store(original_transaction_id, app_account_token)
            .await),
        Request::CancelSubscription => enc(lb.cancel_subscription().await),
        Request::GetSubscriptionInfo => enc(lb.get_subscription_info().await),

        #[cfg(not(target_family = "wasm"))]
        Request::RecentPanic => enc(lb.recent_panic().await),
        #[cfg(not(target_family = "wasm"))]
        Request::WritePanicToFile { error_header, bt } => {
            enc(lb.write_panic_to_file(error_header, bt).await)
        }
        #[cfg(not(target_family = "wasm"))]
        Request::DebugInfo { os_info, check_docs } => enc(lb.debug_info(os_info, check_docs).await),

        Request::ReadDocument { id, user_activity } => {
            enc(lb.read_document(id, user_activity).await)
        }
        Request::WriteDocument { id, content } => enc(lb.write_document(id, &content).await),
        Request::ReadDocumentWithHmac { id, user_activity } => {
            enc(lb.read_document_with_hmac(id, user_activity).await)
        }
        Request::SafeWrite { id, old_hmac, content } => {
            enc(lb.safe_write(id, old_hmac, content).await)
        }

        Request::CreateFile { name, parent, file_type } => {
            enc(lb.create_file(&name, &parent, file_type).await)
        }
        Request::RenameFile { id, new_name } => enc(lb.rename_file(&id, &new_name).await),
        Request::MoveFile { id, new_parent } => enc(lb.move_file(&id, &new_parent).await),
        Request::Delete { id } => enc(lb.delete(&id).await),
        Request::Root => enc(lb.root().await),
        Request::ListMetadatas => enc(lb.list_metadatas().await),
        Request::GetChildren { id } => enc(lb.get_children(&id).await),
        Request::GetAndGetChildrenRecursively { id } => {
            enc(lb.get_and_get_children_recursively(&id).await)
        }
        Request::GetFileById { id } => enc(lb.get_file_by_id(id).await),
        Request::GetFileLinkUrl { id } => enc(lb.get_file_link_url(id).await),
        Request::LocalChanges => enc_plain(lb.local_changes().await),

        Request::TestRepoIntegrity { check_docs } => enc(lb.test_repo_integrity(check_docs).await),

        Request::CreateLinkAtPath { path, target_id } => {
            enc(lb.create_link_at_path(&path, target_id).await)
        }
        Request::CreateAtPath { path } => enc(lb.create_at_path(&path).await),
        Request::GetByPath { path } => enc(lb.get_by_path(&path).await),
        Request::GetPathById { id } => enc(lb.get_path_by_id(id).await),
        Request::ListPaths { filter } => enc(lb.list_paths(filter).await),
        Request::ListPathsWithIds { filter } => enc(lb.list_paths_with_ids(filter).await),

        Request::ShareFile { id, username, mode } => enc(lb.share_file(id, &username, mode).await),
        Request::GetPendingShares => enc(lb.get_pending_shares().await),
        Request::GetPendingShareFiles => enc(lb.get_pending_share_files().await),
        Request::KnownUsernames => enc(lb.known_usernames().await),
        Request::RejectShare { id } => enc(lb.reject_share(&id).await),

        Request::GetUsage => enc(lb.get_usage().await),

        Request::Sync => enc(lb.sync().await),
        Request::Status => enc_plain(lb.status().await),
        Request::GetLastSynced => enc(lb.get_last_synced().await),
        Request::GetLastSyncedHuman => enc(lb.get_last_synced_human().await),
        Request::Subscribe => unreachable!("handle_conn special-cases Subscribe"),
        #[cfg(not(target_family = "wasm"))]
        Request::BuildIndex => enc(lb.build_index().await),
        #[cfg(not(target_family = "wasm"))]
        Request::ReloadSearchIndex => enc(lb.reload_search_index()),
        #[cfg(not(target_family = "wasm"))]
        Request::Search { input, cfg } => enc(lb.search(&input, cfg).await),
    }
}
