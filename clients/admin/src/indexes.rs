use crate::Res;
use clap::Subcommand;

use lb::{Core, ServerIndex};

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum CliIndex {
    OwnedFiles,
    SharedFiles,
    FileChildren,
}

pub fn rebuild(core: &Core, index: CliIndex) -> Res<()> {
    match index {
        CliIndex::OwnedFiles => core.admin_rebuild_index(ServerIndex::OwnedFiles)?,
        CliIndex::SharedFiles => core.admin_rebuild_index(ServerIndex::SharedFiles)?,
        CliIndex::FileChildren => core.admin_rebuild_index(ServerIndex::FileChildren)?,
    }

    Ok(())
}
