use tokio::sync::broadcast::{self, Receiver, Sender};
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
    MetadataChanged(Uuid),

    /// The contents of this document have changed either by this lb
    /// library or as a result of sync
    DocumentWritten(Uuid),

    Sync(SyncIncrement),

    StatusUpdated,
}

impl Default for EventSubs {
    fn default() -> Self {
        let (tx, _) = broadcast::channel::<Event>(10000);
        Self { tx }
    }
}

impl EventSubs {
    pub fn meta_changed(&self, id: Uuid) {
        self.queue(Event::MetadataChanged(id));
    }

    pub fn doc_written(&self, id: Uuid) {
        self.queue(Event::DocumentWritten(id));
    }

    pub fn sync(&self, s: SyncIncrement) {
        self.queue(Event::Sync(s));
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
