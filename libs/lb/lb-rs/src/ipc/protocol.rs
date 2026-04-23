//! Wire-protocol envelope and request/response variants.
//!
//! One [`Request`] / [`Response`] variant per ported `LocalLb` method.
//! Errors travel as full `LbErr` values now that `LbErrKind` derives
//! `Serialize`/`Deserialize` and `LbErr` does too — backtraces survive the
//! IPC hop as their rendered string form.
//!
//! # Sequencing
//!
//! Every `Request` carries a guest-chosen `seq: u64`. The host's matching
//! `Response` carries the same `seq`, so the guest can pair up answers
//! without keeping the connection strictly in lock-step.
//!
//! # Subscriber API (deferred)
//!
//! `Lb::subscribe` returns a `Receiver<Event>` and doesn't fit the
//! request/response shape — it's a long-lived stream of host-pushed
//! messages. A follow-up will extend `Frame` with event/event-end variants
//! (likely tagged by a per-stream id reusing the `Subscribe` request's
//! `seq`) and add an event enum mirroring `service::events::Event`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::account::{Account, Username};
use crate::model::api::{
    AccountFilter, AccountIdentifier, AccountInfo, AdminFileInfoResponse,
    AdminSetUserTierInfo, AdminValidateAccount, AdminValidateServer, ServerIndex,
    StripeAccountTier, SubscriptionInfo,
};
use crate::model::crypto::DecryptedDocument;
use crate::model::errors::{LbResult, Warning};
use crate::model::file::{File, ShareMode};
use crate::model::file_metadata::{DocumentHmac, FileType};
use crate::model::path_ops::Filter;
use crate::service::activity::RankingWeights;
use crate::service::debug::DebugInfo;
use crate::service::usage::UsageMetrics;
use crate::subscribers::search::{SearchConfig, SearchResult};
use crate::subscribers::status::Status;

/// Every byte on the IPC wire is a serialized `Frame`.
#[derive(Debug, Serialize, Deserialize)]
pub enum Frame {
    /// Guest → host: invoke an Lb method.
    Request {
        seq: u64,
        body: Request,
    },
    /// Host → guest: result of a prior `Request` with the same `seq`.
    Response {
        seq: u64,
        body: Response,
    },
}

/// One variant per ported `LocalLb` method. The variant naming mirrors the
/// method name; argument fields use the method's parameter names with
/// owned types where the original method took references.
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    // account
    CreateAccount { username: String, api_url: String, welcome_doc: bool },
    ImportAccount { key: String, api_url: Option<String> },
    ImportAccountPrivateKeyV1 { account: Account },
    ImportAccountPhrase { phrase: [String; 24], api_url: String },
    DeleteAccount,
    // export_account_{private_key,phrase,qr} are sync, return values, and
    // need the Account locally — deferred until the Guest can cache the
    // account at connect time. Until then they're served via the Deref
    // shim, which means Local-only.

    // activity
    SuggestedDocs { settings: RankingWeights },
    ClearSuggested,
    ClearSuggestedId { id: Uuid },
    AppForegrounded,

    // admin
    DisappearAccount { username: String },
    DisappearFile { id: Uuid },
    ListUsers { filter: Option<AccountFilter> },
    GetAccountInfo { identifier: AccountIdentifier },
    AdminValidateAccount { username: String },
    AdminValidateServer,
    AdminFileInfo { id: Uuid },
    RebuildIndex { index: ServerIndex },
    SetUserTier { username: String, info: AdminSetUserTierInfo },

    // billing
    UpgradeAccountStripe { account_tier: StripeAccountTier },
    UpgradeAccountGooglePlay { purchase_token: String, account_id: String },
    UpgradeAccountAppStore { original_transaction_id: String, app_account_token: String },
    CancelSubscription,
    GetSubscriptionInfo,

    // debug
    RecentPanic,
    WritePanicToFile { error_header: String, bt: String },
    DebugInfo { os_info: String, check_docs: bool },

    // documents
    ReadDocument { id: Uuid, user_activity: bool },
    WriteDocument { id: Uuid, content: Vec<u8> },
    ReadDocumentWithHmac { id: Uuid, user_activity: bool },
    SafeWrite { id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8> },

    // file
    CreateFile { name: String, parent: Uuid, file_type: FileType },
    RenameFile { id: Uuid, new_name: String },
    MoveFile { id: Uuid, new_parent: Uuid },
    Delete { id: Uuid },
    Root,
    ListMetadatas,
    GetChildren { id: Uuid },
    GetAndGetChildrenRecursively { id: Uuid },
    GetFileById { id: Uuid },
    GetFileLinkUrl { id: Uuid },
    LocalChanges,

    // integrity
    TestRepoIntegrity { check_docs: bool },

    // keychain — `get_account` is sync and returns `&Account`. Deferred
    // along with the export_account_* methods.

    // path
    CreateLinkAtPath { path: String, target_id: Uuid },
    CreateAtPath { path: String },
    GetByPath { path: String },
    GetPathById { id: Uuid },
    ListPaths { filter: Option<Filter> },
    ListPathsWithIds { filter: Option<Filter> },

    // share
    ShareFile { id: Uuid, username: String, mode: ShareMode },
    GetPendingShares,
    GetPendingShareFiles,
    KnownUsernames,
    RejectShare { id: Uuid },

    // usage
    GetUsage,

    // subscribers
    Sync,
    Status,
    GetLastSyncedHuman,
    Search { input: String, cfg: SearchConfig },
    // get_timestamp_human_string is pure formatting and runs locally on the
    // wrapper regardless of variant — no protocol entry.
}

/// Pairs 1:1 with [`Request`] variants.
#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    // account
    CreateAccount(LbResult<Account>),
    ImportAccount(LbResult<Account>),
    ImportAccountPrivateKeyV1(LbResult<Account>),
    ImportAccountPhrase(LbResult<Account>),
    DeleteAccount(LbResult<()>),

    // activity
    SuggestedDocs(LbResult<Vec<Uuid>>),
    ClearSuggested(LbResult<()>),
    ClearSuggestedId(LbResult<()>),
    /// `app_foregrounded` returns `()` and is fire-and-forget. We still
    /// send a Response so the seq is acknowledged.
    AppForegrounded,

    // admin
    DisappearAccount(LbResult<()>),
    DisappearFile(LbResult<()>),
    ListUsers(LbResult<Vec<Username>>),
    GetAccountInfo(LbResult<AccountInfo>),
    AdminValidateAccount(LbResult<AdminValidateAccount>),
    AdminValidateServer(LbResult<AdminValidateServer>),
    AdminFileInfo(LbResult<AdminFileInfoResponse>),
    RebuildIndex(LbResult<()>),
    SetUserTier(LbResult<()>),

    // billing
    UpgradeAccountStripe(LbResult<()>),
    UpgradeAccountGooglePlay(LbResult<()>),
    UpgradeAccountAppStore(LbResult<()>),
    CancelSubscription(LbResult<()>),
    GetSubscriptionInfo(LbResult<Option<SubscriptionInfo>>),

    // debug
    RecentPanic(LbResult<bool>),
    WritePanicToFile(LbResult<String>),
    DebugInfo(LbResult<DebugInfo>),

    // documents
    ReadDocument(LbResult<DecryptedDocument>),
    WriteDocument(LbResult<()>),
    ReadDocumentWithHmac(LbResult<(Option<DocumentHmac>, DecryptedDocument)>),
    SafeWrite(LbResult<DocumentHmac>),

    // file
    CreateFile(LbResult<File>),
    RenameFile(LbResult<()>),
    MoveFile(LbResult<()>),
    Delete(LbResult<()>),
    Root(LbResult<File>),
    ListMetadatas(LbResult<Vec<File>>),
    GetChildren(LbResult<Vec<File>>),
    GetAndGetChildrenRecursively(LbResult<Vec<File>>),
    GetFileById(LbResult<File>),
    GetFileLinkUrl(LbResult<String>),
    /// `local_changes` returns `Vec<Uuid>` (no Result).
    LocalChanges(Vec<Uuid>),

    // integrity
    TestRepoIntegrity(LbResult<Vec<Warning>>),

    // path
    CreateLinkAtPath(LbResult<File>),
    CreateAtPath(LbResult<File>),
    GetByPath(LbResult<File>),
    GetPathById(LbResult<String>),
    ListPaths(LbResult<Vec<String>>),
    ListPathsWithIds(LbResult<Vec<(Uuid, String)>>),

    // share
    ShareFile(LbResult<()>),
    GetPendingShares(LbResult<Vec<File>>),
    GetPendingShareFiles(LbResult<Vec<File>>),
    KnownUsernames(LbResult<Vec<String>>),
    RejectShare(LbResult<()>),

    // usage
    GetUsage(LbResult<UsageMetrics>),

    // subscribers
    Sync(LbResult<()>),
    Status(Status),
    GetLastSyncedHuman(LbResult<String>),
    Search(LbResult<Vec<SearchResult>>),
}

