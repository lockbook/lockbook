use db_rs::View;
use db_rs::errors::Result;
use db_rs::guard::WriteTx;
use std::ops::{Deref, DerefMut};

use crate::schema::{ServerDb, ServerV6};

#[must_use]
pub struct ServerTx<'a> {
    db: &'a mut ServerDb,
    tx: Option<WriteTx>,
}

impl<'a> ServerTx<'a> {
    pub fn begin(db: &'a mut ServerDb) -> Result<Self> {
        let tx = db.write_tx()?;
        Ok(Self { db, tx: Some(tx) })
    }

    pub fn end(mut self) -> Result<()> {
        self.tx.take().unwrap().end_tx(self.db).map(|_| ())
    }
}

impl Deref for ServerTx<'_> {
    type Target = ServerV6;

    fn deref(&self) -> &Self::Target {
        &self.db.schema
    }
}

impl DerefMut for ServerTx<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.db.schema
    }
}

impl Drop for ServerTx<'_> {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            if let Err(error) = tx.end_tx(self.db) {
                tracing::error!(?error, "failed to flush database transaction on drop");
            }
        }
    }
}
