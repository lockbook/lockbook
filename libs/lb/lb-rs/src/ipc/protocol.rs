//! Wire-protocol envelope.
//!
//! [`Request`] is a typed enum — one variant per ported method — so the
//! server's dispatch `match` is exhaustive at compile time and can't drift
//! out of sync with new variants. Responses, on the other hand, are
//! type-erased: the host writes a bincode-encoded `LbResult<Out>` and the
//! client side ([`crate::ipc::client::RemoteLb::call`]) is parameterized by
//! the expected `Out`. If the two sides disagree on `Out`, bincode fails
//! and the error surfaces as `LbErrKind::Unexpected`.
//!
//! The asymmetry is deliberate. Inputs benefit most from compile-time
//! checks — they tend to grow, get reordered, and accumulate fields.
//! Outputs are usually one return type per method and are easy to keep in
//! sync at the call site.
//!
//! # Sequencing
//!
//! Every `Request` carries a guest-chosen `seq: u64`; the host's matching
//! `Response` carries the same `seq`, so calls multiplex over one
//! connection without enforcing lock-step ordering.
//!
//! # Subscriber API
//!
//! `Lb::subscribe` returns a `Receiver<Event>` — a long-lived stream of
//! host-pushed events. The wire shape: a guest sends `Request::Subscribe`,
//! the host acks with the standard `Response`, and then asynchronously
//! pushes [`Frame::Event`] messages tagged with `stream_seq` (matching the
//! Subscribe request's seq for future multi-stream support). When the
//! host's subscription closes (host shutdown, channel error) it sends
//! [`Frame::EventEnd`] as a courtesy.

use serde::{Deserialize, Serialize};
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

/// Every byte on the IPC wire is a serialized `Frame`.
#[derive(Debug, Serialize, Deserialize)]
pub enum Frame {
    /// Guest → host: invoke a method.
    Request { seq: u64, body: Request },
    /// Host → guest: bincode-encoded `LbResult<Out>` where `Out` is whatever
    /// the originating call asked for. Type-erased on the wire by design.
    Response { seq: u64, output: Vec<u8> },
    /// Host → guest: one event from a previously-opened subscription.
    /// `stream_seq` matches the originating `Request::Subscribe`'s seq, so
    /// future multi-stream multiplexing on a single connection is possible
    /// without changing the wire shape.
    Event { stream_seq: u64, body: Event },
    /// Host → guest: the subscription is over (host shutting down, the
    /// underlying broadcast errored, etc.). After this no more
    /// `Frame::Event` will arrive for `stream_seq`.
    EventEnd { stream_seq: u64 },
}

/// One variant per ported `LocalLb` method. Adding a method means one
/// variant here plus one match arm in [`crate::ipc::server::dispatch`] —
/// the compiler enforces both sides stay in sync.
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    // account
    CreateAccount { username: String, api_url: String, welcome_doc: bool },
    ImportAccount { key: String, api_url: Option<String> },
    ImportAccountPrivateKeyV1 { account: Account },
    ImportAccountPhrase { phrase: [String; 24], api_url: String },
    DeleteAccount,
    /// Fetch the host's current Account, if any. The guest calls this once
    /// at connect time to seed its sync `get_account()` cache, and again
    /// implicitly after each successful create/import on the wrapper.
    GetAccount,
    // export_account_{private_key,phrase,qr} are still served by the
    // Deref shim — they're pure compute on the cached Account and could
    // move onto the Lb wrapper directly in a future pass.

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

    // debug (cfg!=wasm)
    #[cfg(not(target_family = "wasm"))]
    RecentPanic,
    #[cfg(not(target_family = "wasm"))]
    WritePanicToFile { error_header: String, bt: String },
    #[cfg(not(target_family = "wasm"))]
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
    GetLastSynced,
    GetLastSyncedHuman,
    /// Open a long-lived event stream on this connection. The host acks
    /// with `Response { seq, output: Ok(()) }` and starts pushing
    /// `Frame::Event { stream_seq: seq, .. }` until the broadcast closes.
    Subscribe,
    #[cfg(not(target_family = "wasm"))]
    Search { input: String, cfg: SearchConfig },
    // get_timestamp_human_string is pure formatting and runs locally on the
    // wrapper regardless of variant — no protocol entry.
}
