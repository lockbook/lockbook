use crate::Res;

use lockbook_core::{Core, ServerIndex};
use structopt::StructOpt;

#[derive(Debug, PartialEq, Eq, StructOpt)]
pub enum CliIndex {
    OwnedFiles,
}

pub fn rebuild(core: &Core, index: CliIndex) -> Res<()> {
    match index {
        CliIndex::OwnedFiles => core.admin_rebuild_index(ServerIndex::OwnedFiles)?,
    }

    Ok(())
}
