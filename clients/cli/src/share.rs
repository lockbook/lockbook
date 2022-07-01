use lockbook_core::{Core, ShareMode, Uuid};

use crate::error::CliError;

pub fn share(core: &Core, path: &str, username: &str) -> Result<(), CliError> {
    let id = core.get_by_path(path).unwrap().id;
    core.share_file(id, username, ShareMode::Write).unwrap(); // todo(sharing): handle errors, other share modes
    Ok(())
}

pub fn get_pending_shares(core: &Core) -> Result<(), CliError> {
    let pending_shares = core.get_pending_shares().unwrap(); // todo(sharing): handle errors
    for share in pending_shares {
        println!("{}", share.id); // todo(sharing): better ux
    }
    Ok(())
}

pub fn new_link(core: &Core, link_path: &str, target_id: &str) -> Result<(), CliError> {
    core.create_link_at_path(link_path, Uuid::parse_str(target_id).unwrap())
        .unwrap();
    Ok(())
}

pub fn delete_pending_share(core: &Core, target_id: &str) -> Result<(), CliError> {
    core.delete_pending_share(Uuid::parse_str(target_id).unwrap())
        .unwrap(); // todo(sharing): handle errors
    Ok(())
}
