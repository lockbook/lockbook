//! Wire-protocol envelope and request/response placeholders.
//!
//! Stage 2 defines the envelope ([`Frame`]) and stub variants for
//! [`Request`] and [`Response`]. Stage 3 replaces the `__StagePlaceholder`
//! variants with one variant per public `Lb` method and wires server
//! dispatch / `RemoteLb` forwarders against them.
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
//! `seq`) and add an event enum mirroring `service::events::Event`. Until
//! then the protocol covers only request/response.

use serde::{Deserialize, Serialize};

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

/// Stage 3 replaces `__StagePlaceholder` with one variant per `Lb` method,
/// e.g. `CreateFile { name: String, parent: Uuid, ft: FileType }`.
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    #[doc(hidden)]
    __StagePlaceholder,
}

/// Pairs 1:1 with [`Request`] variants. Each Stage 3 variant wraps the
/// corresponding method's `LbResult<T>`.
#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    #[doc(hidden)]
    __StagePlaceholder,
}
