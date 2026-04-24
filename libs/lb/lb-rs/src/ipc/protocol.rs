use std::io;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

use crate::model::account::Account;
use crate::model::api::{
    AccountFilter, AccountIdentifier, AdminSetUserTierInfo, ServerIndex, StripeAccountTier,
};
use crate::model::file::ShareMode;
use crate::model::file_metadata::{DocumentHmac, FileType};
use crate::model::path_ops::Filter;
use crate::service::activity::RankingWeights;
use crate::service::events::Event;
#[cfg(not(target_family = "wasm"))]
use crate::subscribers::search::SearchConfig;

#[derive(Debug, Serialize, Deserialize)]
pub enum Frame {
    Request { seq: u64, body: Request },
    Response { seq: u64, output: Vec<u8> },
    Event { stream_seq: u64, body: Event },
    EventEnd { stream_seq: u64 },
}

impl Frame {
    pub async fn read<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Self> {
        let mut len_buf = [0u8; 4];
        r.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        r.read_exact(&mut buf).await?;
        bincode::deserialize(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub async fn write<W: AsyncWrite + Unpin>(&self, w: &mut W) -> io::Result<()> {
        let bytes =
            bincode::serialize(self).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let len: u32 = bytes.len().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame {} bytes does not fit in a u32 length prefix", bytes.len()),
            )
        })?;
        w.write_all(&len.to_le_bytes()).await?;
        w.write_all(&bytes).await?;
        w.flush().await
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    CreateAccount {
        username: String,
        api_url: String,
        welcome_doc: bool,
    },
    ImportAccount {
        key: String,
        api_url: Option<String>,
    },
    ImportAccountPrivateKeyV1 {
        account: Account,
    },
    ImportAccountPhrase {
        phrase: Vec<String>,
        api_url: String,
    },
    DeleteAccount,
    GetAccount,

    SuggestedDocs {
        settings: RankingWeights,
    },
    ClearSuggested,
    ClearSuggestedId {
        id: Uuid,
    },
    AppForegrounded,

    DisappearAccount {
        username: String,
    },
    DisappearFile {
        id: Uuid,
    },
    ListUsers {
        filter: Option<AccountFilter>,
    },
    GetAccountInfo {
        identifier: AccountIdentifier,
    },
    AdminValidateAccount {
        username: String,
    },
    AdminValidateServer,
    AdminFileInfo {
        id: Uuid,
    },
    RebuildIndex {
        index: ServerIndex,
    },
    SetUserTier {
        username: String,
        info: AdminSetUserTierInfo,
    },

    UpgradeAccountStripe {
        account_tier: StripeAccountTier,
    },
    UpgradeAccountGooglePlay {
        purchase_token: String,
        account_id: String,
    },
    UpgradeAccountAppStore {
        original_transaction_id: String,
        app_account_token: String,
    },
    CancelSubscription,
    GetSubscriptionInfo,

    #[cfg(not(target_family = "wasm"))]
    RecentPanic,
    #[cfg(not(target_family = "wasm"))]
    WritePanicToFile {
        error_header: String,
        bt: String,
    },
    #[cfg(not(target_family = "wasm"))]
    DebugInfo {
        os_info: String,
        check_docs: bool,
    },

    ReadDocument {
        id: Uuid,
        user_activity: bool,
    },
    WriteDocument {
        id: Uuid,
        content: Vec<u8>,
    },
    ReadDocumentWithHmac {
        id: Uuid,
        user_activity: bool,
    },
    SafeWrite {
        id: Uuid,
        old_hmac: Option<DocumentHmac>,
        content: Vec<u8>,
    },

    CreateFile {
        name: String,
        parent: Uuid,
        file_type: FileType,
    },
    RenameFile {
        id: Uuid,
        new_name: String,
    },
    MoveFile {
        id: Uuid,
        new_parent: Uuid,
    },
    Delete {
        id: Uuid,
    },
    Root,
    ListMetadatas,
    GetChildren {
        id: Uuid,
    },
    GetAndGetChildrenRecursively {
        id: Uuid,
    },
    GetFileById {
        id: Uuid,
    },
    GetFileLinkUrl {
        id: Uuid,
    },
    LocalChanges,

    TestRepoIntegrity {
        check_docs: bool,
    },

    CreateLinkAtPath {
        path: String,
        target_id: Uuid,
    },
    CreateAtPath {
        path: String,
    },
    GetByPath {
        path: String,
    },
    GetPathById {
        id: Uuid,
    },
    ListPaths {
        filter: Option<Filter>,
    },
    ListPathsWithIds {
        filter: Option<Filter>,
    },

    ShareFile {
        id: Uuid,
        username: String,
        mode: ShareMode,
    },
    GetPendingShares,
    GetPendingShareFiles,
    KnownUsernames,
    RejectShare {
        id: Uuid,
    },

    PinFile {
        id: Uuid,
    },
    UnpinFile {
        id: Uuid,
    },
    ListPinned,

    GetUsage,

    Sync,
    Status,
    GetLastSynced,
    GetLastSyncedHuman,
    Subscribe,
    #[cfg(not(target_family = "wasm"))]
    BuildIndex,
    #[cfg(not(target_family = "wasm"))]
    ReloadSearchIndex,
    #[cfg(not(target_family = "wasm"))]
    Search {
        input: String,
        cfg: SearchConfig,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn frame_round_trip() {
        let (mut a, mut b) = duplex(64 * 1024);
        let frame = Frame::Request { seq: 7, body: Request::Sync };
        frame.write(&mut a).await.unwrap();
        let got = Frame::read(&mut b).await.unwrap();
        assert!(matches!(got, Frame::Request { seq: 7, body: Request::Sync }));
    }
}
