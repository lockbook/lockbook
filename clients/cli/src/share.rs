use crate::selector::select_meta;
use crate::CliError;
use lockbook_core::{Core, FileType, ShareMode, Uuid};
use structopt::StructOpt;

#[derive(Debug, PartialEq, Eq, StructOpt)]
pub enum Share {
    /// Share a specified document with a user. If neither a path or id
    /// is specified, an interactivve selector will be launched
    New {
        path: Option<String>,

        #[structopt(short, long)]
        id: Option<Uuid>,

        name: String,
    },

    /// List share requests for you, from other people
    Pending,

    /// Accept a share, and place it within your lockbook
    Accept { id: Uuid, dest: Option<String>, id_dest: Option<Uuid> },
}

pub fn share(core: &Core, share: Share) -> Result<(), CliError> {
    match share {
        Share::New { path, id, name } => new(core, path, id, name),
        Share::Pending => pending(core),
        Share::Accept { id, dest, id_dest } => accept(core, id, dest, id_dest),
    }
}

fn new(core: &Core, path: Option<String>, id: Option<Uuid>, name: String) -> Result<(), CliError> {
    let file = select_meta(core, path, id, None, Some("Select a file to share"))?;
    core.share_file(file.id, &name, ShareMode::Write).unwrap(); // TODO
    Ok(())
}

fn pending(core: &Core) -> Result<(), CliError> {
    for file in core.get_pending_shares()? {
        println!("{:#?}", file);
    }

    Ok(())
}

fn accept(
    core: &Core, id: Uuid, dest_path: Option<String>, dest_id: Option<Uuid>,
) -> Result<(), CliError> {
    let share = core
        .get_pending_shares()?
        .into_iter()
        .find(|f| f.id == id)
        .expect("Share file not found");

    let dest = select_meta(
        core,
        dest_path,
        dest_id,
        Some(FileType::Folder),
        Some("Select a destination to place the shared file."),
    )?;

    core.create_file(&share.name, dest.id, FileType::Link { target: share.id })
        .unwrap();

    Ok(())
}
