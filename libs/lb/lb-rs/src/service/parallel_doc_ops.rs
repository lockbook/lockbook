use crate::service::api_service::Requester;
use crate::service::sync_service::SyncContext;
use crate::{LbError, LbResult};
use lockbook_shared::account::Account;
use lockbook_shared::api::GetDocRequest;
use lockbook_shared::document_repo::DocumentService;
use lockbook_shared::file_metadata::DocumentHmac;
use std::cmp::min;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;

pub type Doc = (Uuid, DocumentHmac);
pub type Docs = Vec<Doc>;

enum Status {
    Started(Uuid),
    Failed(LbError),
}

impl<C: Requester, D: DocumentService> SyncContext<C, D> {
    pub(crate) fn para_pull(&mut self, docs: Docs) -> LbResult<()> {
        match docs.as_slice() {
            [] => return Ok(()),
            [(id, hmac)] => {
                self.file_msg(*id, &format!("Downloading file 1 of 1.")); // todo: add name
                return Self::fetch_doc(&self.client, &self.docs, &self.account, (*id, *hmac));
            }
            _ => {}
        }

        let mut rx_count = 0;
        let rx_total = docs.len();

        let work_queue = Arc::new(Mutex::new(docs));
        let (msg_tx, msg_rx) = channel();

        let workers = min(rx_total, num_cpus::get());

        for _ in 0..workers {
            let work_queue = work_queue.clone();
            let client = C::default();
            let docs = self.docs.clone();
            let account = self.account.clone();
            let msg_tx = msg_tx.clone();

            thread::spawn(move || loop {
                let mut queue = work_queue.lock()?;

                let work = match queue.pop() {
                    None => return Ok::<(), LbError>(()),
                    Some(work) => work,
                };
                drop(queue);

                msg_tx.send(Status::Started(work.0))?;
                if let Err(e) = Self::fetch_doc(&client, &docs, &account, work) {
                    // have co-workers stop working
                    work_queue.lock()?.clear();

                    // communicate that we failed
                    msg_tx.send(Status::Failed(e))?;

                    return Ok::<(), LbError>(());
                }
            });
        }

        drop(msg_tx); // without this line, the following loop will never terminate
        for status in msg_rx {
            rx_count += 1;
            match status {
                Status::Started(id) => {
                    self.file_msg(id, &format!("Downloading file {rx_count} of {rx_total}."))
                }
                Status::Failed(e) => return Err(e),
            }
            // if rx_count == rx_total {
            //     return Ok(());
            // }
        }
        ///________________________________________________________
        // Executed in   82.99 secs    fish           external
        //    usr time    7.16 secs  221.00 micros    7.16 secs
        //    sys time    2.88 secs  709.00 micros    2.88 secs
        Ok(())
    }

    pub(crate) fn fetch_doc(c: &C, d: &D, account: &Account, doc: Doc) -> LbResult<()> {
        let id = doc.0;
        let hmac = doc.1;

        let remote_document = c.request(account, GetDocRequest { id, hmac })?;
        d.insert(&id, Some(&hmac), &remote_document.content)?;

        Ok(())
    }
}
