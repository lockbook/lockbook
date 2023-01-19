use crate::Res;

use structopt::StructOpt;
use lockbook_core::{Core, ServerIndex};

#[derive(Debug, PartialEq, Eq, StructOpt)]
pub enum CliIndex {
    OwnedFiles
}

fn rebuild(core: &Core, index: CliIndex) -> Res<()> {
    match index {
        CliIndex::OwnedFiles => core.admin_rebuild_index(ServerIndex::OwnedFiles)?
    }

    Ok(())
}