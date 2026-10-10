use db_rs::View;
pub use tokio::sync::broadcast::{self, Receiver, Sender};
use tracing::*;
use uuid::Uuid;

use crate::io::CoreV5;
use crate::{Lb, LbErrKind};

#[derive(Clone)]
pub struct EventSubs {
    tx: Sender<Event>,
}

#[derive(Clone, Debug)]
pub enum Event {
    /// A metadata for a given id or it's descendants changed. The id returned
    /// may be deleted. Updates to document contents will not cause this
    /// message to be sent (unless a document was deleted).
    MetadataChanged(Actor),

    /// The contents of this document have changed either by this lb
    /// library or as a result of sync
    DocumentWritten(Uuid, Actor),

    /// Changes from another process have been applied to our local views.
    /// Cached metadata and document contents may need to be reloaded.
    IpcChangesApplied,

    PendingSharesChanged,

    Sync(SyncIncrement),

    StatusUpdated,

    UserSignedIn,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Actor {
    /// A write initiated locally. The id identifies the writer (e.g. a
    /// workspace instance) so subscribers can tell their own writes apart
    /// from writes by other workspaces or tools sharing this lb.
    User(Option<Uuid>),
    Sync,
}

impl Default for EventSubs {
    fn default() -> Self {
        let (tx, _) = broadcast::channel::<Event>(10000);
        Self { tx }
    }
}

impl EventSubs {
    pub(crate) fn pending_shares_changed(&self) {
        self.queue(Event::PendingSharesChanged);
    }

    pub(crate) fn meta_changed(&self, actor: Actor) {
        self.queue(Event::MetadataChanged(actor));
    }

    pub(crate) fn doc_written(&self, id: Uuid, actor: Actor) {
        self.queue(Event::DocumentWritten(id, actor));
    }

    pub(crate) fn sync_update(&self, s: SyncIncrement) {
        self.queue(Event::Sync(s));
    }

    pub(crate) fn status_updated(&self) {
        self.queue(Event::StatusUpdated);
    }

    /// executed after root and account are created
    pub(crate) fn signed_in(&self) {
        self.queue(Event::UserSignedIn);
    }

    fn queue(&self, evt: Event) {
        if let Err(e) = self.tx.send(evt.clone()) {
            error!(?evt, ?e, "could not queue");
        }
    }
}

impl Lb {
    pub(crate) async fn notify_catch_up(&self, db: &CoreV5, previous_seq: u64) {
        if db.account.last_modified() > previous_seq && self.get_account().is_err() {
            if let Some(account) = db.account.as_ref() {
                self.keychain.cache_account(account.clone()).await.unwrap();
                self.events.signed_in();
            }
        }

        let changed = [
            db.account.last_modified(),
            db.root.last_modified(),
            db.base_metadata.last_modified(),
            db.local_metadata.last_modified(),
            db.pub_key_lookup.last_modified(),
            db.pinned_files.last_modified(),
            db.last_synced.last_modified(),
        ]
        .into_iter()
        .any(|seq| seq > previous_seq);

        // Read activity is deliberately excluded: reloading documents records
        // more reads, which must not trigger reloads in the other process.
        if changed {
            self.events.queue(Event::IpcChangesApplied);
        }
    }

    pub fn subscribe(&self) -> Receiver<Event> {
        self.events.tx.subscribe()
    }
}

#[derive(Debug, Clone)]
pub enum SyncIncrement {
    SyncStarted,
    PullingDocument(Uuid, bool),
    PushingDocument(Uuid, bool),
    SyncFinished(Option<LbErrKind>),
}
