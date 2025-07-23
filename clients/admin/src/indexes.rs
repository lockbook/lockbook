use crate::Res;
use clap::Subcommand;
use lb::blocking::Lb;
use lb::model::api::ServerIndex;

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum CliIndex {
    OwnedFiles,
    SharedFiles,
    FileChildren,
}

pub fn rebuild(lb: &Lb, index: CliIndex) -> Res<()> {
    match index {
        CliIndex::OwnedFiles => lb.admin_rebuild_index(ServerIndex::OwnedFiles)?,
        CliIndex::SharedFiles => lb.admin_rebuild_index(ServerIndex::SharedFiles)?,
        CliIndex::FileChildren => lb.admin_rebuild_index(ServerIndex::FileChildren)?,
    }

    Ok(())
}
