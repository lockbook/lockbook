use tokio::sync::broadcast::{self, Receiver, Sender};
use uuid::Uuid;

use crate::Lb;

#[derive(Clone)]
pub struct EventSubs {
    tx: Sender<Event>,
}

#[derive(Clone, Copy)]
pub enum Event {
    MetadataChanged(Uuid),
    FileRemoved(Uuid),
    DocumentWritten(Uuid),
}

impl Default for EventSubs {
    fn default() -> Self {
        let (tx, _) = broadcast::channel::<Event>(1000);
        Self { tx }
    }
}

impl EventSubs {
    pub fn doc_written(&self, id: Uuid) {
        // todo warn!
        let _ = self.tx.send(Event::DocumentWritten(id));
    }
}

impl Lb {
    pub fn subscribe(&self) -> Receiver<Event> {
        self.events.tx.subscribe()
    }
}
