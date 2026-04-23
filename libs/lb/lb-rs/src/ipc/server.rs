//! Host-side IPC server.
//!
//! Accepts UDS connections and dispatches each [`Request`] against the
//! shared [`LocalLb`]. Mirrors every variant in [`crate::ipc::protocol`]
//! one-to-one — adding a new method means a `Request` variant + a
//! `Response` variant + a match arm here + a forwarder on `Lb`.

use std::io;
use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::net::{UnixListener, UnixStream};

use crate::LocalLb;
use crate::ipc::frame::{read_frame, write_frame};
use crate::ipc::protocol::{Frame, Request, Response};

/// Run the accept loop until the listener errors fatally. Spawns a task per
/// accepted connection.
///
/// `lb` is shared (`Arc`) across all connections — they all dispatch into
/// the same in-process state, exactly as if every guest were a thread in
/// the host.
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

async fn handle_conn(mut stream: UnixStream, lb: Arc<LocalLb>) -> io::Result<()> {
    let (mut r, mut w) = stream.split();
    loop {
        let frame_bytes = read_frame(&mut r).await?;
        let frame: Frame = bincode::deserialize(&frame_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        match frame {
            Frame::Request { seq, body } => {
                let response_body = dispatch(&lb, body).await;
                let response = Frame::Response { seq, body: response_body };
                let bytes = bincode::serialize(&response)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                write_frame(&mut w, &bytes).await?;
                w.flush().await?;
            }
            Frame::Response { .. } => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "guest sent a host-only frame",
                ));
            }
        }
    }
}

/// Dispatch a single guest [`Request`] against the host's [`LocalLb`].
async fn dispatch(lb: &LocalLb, req: Request) -> Response {
    match req {
        // -- account ------------------------------------------------------
        Request::CreateAccount { username, api_url, welcome_doc } => Response::CreateAccount(
            lb.create_account(&username, &api_url, welcome_doc).await,
        ),
        Request::ImportAccount { key, api_url } => {
            Response::ImportAccount(lb.import_account(&key, api_url.as_deref()).await)
        }
        Request::ImportAccountPrivateKeyV1 { account } => {
            Response::ImportAccountPrivateKeyV1(lb.import_account_private_key_v1(account).await)
        }
        Request::ImportAccountPhrase { phrase, api_url } => {
            let phrase_refs: [&str; 24] = std::array::from_fn(|i| phrase[i].as_str());
            Response::ImportAccountPhrase(lb.import_account_phrase(phrase_refs, &api_url).await)
        }
        Request::DeleteAccount => Response::DeleteAccount(lb.delete_account().await),

        // -- activity -----------------------------------------------------
        Request::SuggestedDocs { settings } => {
            Response::SuggestedDocs(lb.suggested_docs(settings).await)
        }
        Request::ClearSuggested => Response::ClearSuggested(lb.clear_suggested().await),
        Request::ClearSuggestedId { id } => {
            Response::ClearSuggestedId(lb.clear_suggested_id(id).await)
        }
        Request::AppForegrounded => {
            lb.app_foregrounded();
            Response::AppForegrounded
        }

        // -- admin --------------------------------------------------------
        Request::DisappearAccount { username } => {
            Response::DisappearAccount(lb.disappear_account(&username).await)
        }
        Request::DisappearFile { id } => Response::DisappearFile(lb.disappear_file(id).await),
        Request::ListUsers { filter } => Response::ListUsers(lb.list_users(filter).await),
        Request::GetAccountInfo { identifier } => {
            Response::GetAccountInfo(lb.get_account_info(identifier).await)
        }
        Request::AdminValidateAccount { username } => {
            Response::AdminValidateAccount(lb.validate_account(&username).await)
        }
        Request::AdminValidateServer => Response::AdminValidateServer(lb.validate_server().await),
        Request::AdminFileInfo { id } => Response::AdminFileInfo(lb.file_info(id).await),
        Request::RebuildIndex { index } => Response::RebuildIndex(lb.rebuild_index(index).await),
        Request::SetUserTier { username, info } => {
            Response::SetUserTier(lb.set_user_tier(&username, info).await)
        }

        // -- billing ------------------------------------------------------
        Request::UpgradeAccountStripe { account_tier } => {
            Response::UpgradeAccountStripe(lb.upgrade_account_stripe(account_tier).await)
        }
        Request::UpgradeAccountGooglePlay { purchase_token, account_id } => {
            Response::UpgradeAccountGooglePlay(
                lb.upgrade_account_google_play(&purchase_token, &account_id)
                    .await,
            )
        }
        Request::UpgradeAccountAppStore { original_transaction_id, app_account_token } => {
            Response::UpgradeAccountAppStore(
                lb.upgrade_account_app_store(original_transaction_id, app_account_token)
                    .await,
            )
        }
        Request::CancelSubscription => {
            Response::CancelSubscription(lb.cancel_subscription().await)
        }
        Request::GetSubscriptionInfo => {
            Response::GetSubscriptionInfo(lb.get_subscription_info().await)
        }

        // -- debug --------------------------------------------------------
        Request::RecentPanic => Response::RecentPanic(lb.recent_panic().await),
        Request::WritePanicToFile { error_header, bt } => {
            Response::WritePanicToFile(lb.write_panic_to_file(error_header, bt).await)
        }
        Request::DebugInfo { os_info, check_docs } => {
            Response::DebugInfo(lb.debug_info(os_info, check_docs).await)
        }

        // -- documents ----------------------------------------------------
        Request::ReadDocument { id, user_activity } => {
            Response::ReadDocument(lb.read_document(id, user_activity).await)
        }
        Request::WriteDocument { id, content } => {
            Response::WriteDocument(lb.write_document(id, &content).await)
        }
        Request::ReadDocumentWithHmac { id, user_activity } => {
            Response::ReadDocumentWithHmac(lb.read_document_with_hmac(id, user_activity).await)
        }
        Request::SafeWrite { id, old_hmac, content } => {
            Response::SafeWrite(lb.safe_write(id, old_hmac, content).await)
        }

        // -- file ---------------------------------------------------------
        Request::CreateFile { name, parent, file_type } => {
            Response::CreateFile(lb.create_file(&name, &parent, file_type).await)
        }
        Request::RenameFile { id, new_name } => {
            Response::RenameFile(lb.rename_file(&id, &new_name).await)
        }
        Request::MoveFile { id, new_parent } => {
            Response::MoveFile(lb.move_file(&id, &new_parent).await)
        }
        Request::Delete { id } => Response::Delete(lb.delete(&id).await),
        Request::Root => Response::Root(lb.root().await),
        Request::ListMetadatas => Response::ListMetadatas(lb.list_metadatas().await),
        Request::GetChildren { id } => Response::GetChildren(lb.get_children(&id).await),
        Request::GetAndGetChildrenRecursively { id } => {
            Response::GetAndGetChildrenRecursively(lb.get_and_get_children_recursively(&id).await)
        }
        Request::GetFileById { id } => Response::GetFileById(lb.get_file_by_id(id).await),
        Request::GetFileLinkUrl { id } => Response::GetFileLinkUrl(lb.get_file_link_url(id).await),
        Request::LocalChanges => Response::LocalChanges(lb.local_changes().await),

        // -- integrity ----------------------------------------------------
        Request::TestRepoIntegrity { check_docs } => {
            Response::TestRepoIntegrity(lb.test_repo_integrity(check_docs).await)
        }

        // -- path ---------------------------------------------------------
        Request::CreateLinkAtPath { path, target_id } => {
            Response::CreateLinkAtPath(lb.create_link_at_path(&path, target_id).await)
        }
        Request::CreateAtPath { path } => Response::CreateAtPath(lb.create_at_path(&path).await),
        Request::GetByPath { path } => Response::GetByPath(lb.get_by_path(&path).await),
        Request::GetPathById { id } => Response::GetPathById(lb.get_path_by_id(id).await),
        Request::ListPaths { filter } => Response::ListPaths(lb.list_paths(filter).await),
        Request::ListPathsWithIds { filter } => {
            Response::ListPathsWithIds(lb.list_paths_with_ids(filter).await)
        }

        // -- share --------------------------------------------------------
        Request::ShareFile { id, username, mode } => {
            Response::ShareFile(lb.share_file(id, &username, mode).await)
        }
        Request::GetPendingShares => Response::GetPendingShares(lb.get_pending_shares().await),
        Request::GetPendingShareFiles => {
            Response::GetPendingShareFiles(lb.get_pending_share_files().await)
        }
        Request::KnownUsernames => Response::KnownUsernames(lb.known_usernames().await),
        Request::RejectShare { id } => Response::RejectShare(lb.reject_share(&id).await),

        // -- usage --------------------------------------------------------
        Request::GetUsage => Response::GetUsage(lb.get_usage().await),

        // -- subscribers --------------------------------------------------
        Request::Sync => Response::Sync(lb.sync().await),
        Request::Status => Response::Status(lb.status().await),
        Request::GetLastSyncedHuman => {
            Response::GetLastSyncedHuman(lb.get_last_synced_human().await)
        }
        Request::Search { input, cfg } => Response::Search(lb.search(&input, cfg).await),
    }
}
