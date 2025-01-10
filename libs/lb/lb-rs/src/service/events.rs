use tokio::sync::broadcast::{self, Receiver, Sender};
use tracing::*;
use uuid::Uuid;

use crate::Lb;

#[derive(Clone)]
pub struct EventSubs {
    tx: Sender<Event>,
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// If any document or folder is birthed by this lb library
    /// new files as a result of sync are communicated as MetadataChanged
    /// documents that have new contents are communicated as DocumentWritten
    NewFile(Uuid),

    /// If metadata changes either this lb library, or a sync
    /// document content changes are communicated as DocumentWritten
    /// if there is any uncertainty parents are returned rather than their
    /// children (for example to make the implementation of create_at_path
    /// simpler)
    MetadataChanged(Uuid),

    DocumentRemoved(Uuid),
    FolderRemoved(Uuid),
    DocumentWritten(Uuid),
}

impl Default for EventSubs {
    fn default() -> Self {
        let (tx, _) = broadcast::channel::<Event>(1000);
        Self { tx }
    }
}

impl EventSubs {
    pub fn new_file(&self, id: Uuid) {
        self.queue(Event::NewFile(id));
    }

    pub fn meta_changed(&self, id: Uuid) {
        self.queue(Event::MetadataChanged(id));
    }

    pub fn doc_removed(&self, id: Uuid) {
        self.queue(Event::DocumentRemoved(id));
    }

    pub fn folder_removed(&self, id: Uuid) {
        self.queue(Event::FolderRemoved(id));
    }

    pub fn doc_written(&self, id: Uuid) {
        self.queue(Event::DocumentWritten(id));
    }

    fn queue(&self, evt: Event) {
        if let Err(e) = self.tx.send(evt) {
            error!(?evt, ?e, "could not queue");
        }
    }
}

impl Lb {
    pub fn subscribe(&self) -> Receiver<Event> {
        self.events.tx.subscribe()
    }
}
