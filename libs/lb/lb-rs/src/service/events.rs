pub use tokio::sync::broadcast::{self, Receiver, Sender};
use tracing::*;
use uuid::Uuid;

use crate::Lb;

use super::sync::SyncIncrement;

#[derive(Clone)]
pub struct EventSubs {
    tx: Sender<Event>,
}

#[derive(Clone, Debug)]
pub enum Event {
    /// A metadata for a given id or it's descendants changed. The id returned
    /// may be deleted. Updates to document contents will not cause this
    /// message to be sent (unless a document was deleted).
    MetadataChanged,

    /// The contents of this document have changed either by this lb
    /// library or as a result of sync
    DocumentWritten(Uuid, Option<Actor>),

    PendingSharesChanged,

    Sync(SyncIncrement),

    StatusUpdated,
}

#[derive(Debug, Clone)]
pub enum Actor {
    Workspace,
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

    pub(crate) fn meta_changed(&self) {
        self.queue(Event::MetadataChanged);
    }

    pub(crate) fn doc_written(&self, id: Uuid, actor: Option<Actor>) {
        self.queue(Event::DocumentWritten(id, actor));
    }

    pub(crate) fn sync(&self, s: SyncIncrement) {
        self.queue(Event::Sync(s));
    }

    pub(crate) fn status_updated(&self) {
        self.queue(Event::StatusUpdated);
    }

    fn queue(&self, evt: Event) {
        if let Err(e) = self.tx.send(evt.clone()) {
            error!(?evt, ?e, "could not queue");
        }
    }
}

impl Lb {
    pub fn subscribe(&self) -> Receiver<Event> {
        self.events.tx.subscribe()
    }
}
