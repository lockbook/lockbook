use lockbook_shared::file::{File, ShareMode};
use uuid::Uuid;

use crate::{CoreResult, RequestContext};

impl RequestContext<'_, '_> {
    pub fn share_file(
        &mut self, id: Uuid, sharee_username: &str, mode: ShareMode,
    ) -> CoreResult<()> {
        todo!()
    }

    pub fn get_pending_shares(&mut self) -> CoreResult<Vec<File>> {
        todo!()
    }

    pub fn delete_pending_share(&mut self, id: Uuid) -> CoreResult<()> {
        todo!()
    }
}
