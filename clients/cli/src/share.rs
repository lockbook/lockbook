use lb::Core;
use lb::Uuid;

use crate::completions::DynValueName::{LbAnyPath, LbFolderPath, PendingShareId};
use crate::maybe_get_by_path;
use crate::resolve_target_to_id;
use crate::CliError;
use crate::ID_PREFIX_LEN;

#[derive(clap::Subcommand, Debug)]
pub enum ShareCmd {
    /// share a file with another lockbook user
    New {
        /// ID or path of the file you will share
        #[arg(value_name = LbAnyPath.as_ref())]
        target: String,
        /// username of who you would like to share with
        username: String,
        /// read-only (the other user will not be able to edit the shared file)
        #[clap(long = "ro")]
        read_only: bool,
    },
    /// list pending shares
    Pending {
        /// display full file IDs instead of prefixes
        #[clap(long)]
        full_ids: bool,
    },
    /// accept a pending by adding it to your file tree
    Accept {
        /// ID (full or prefix) of a pending share
        #[arg(value_name = PendingShareId.as_ref())]
        target: String,
        /// lockbook file path or ID
        #[clap(default_value = "/", value_name = LbFolderPath.as_ref())]
        dest: String,
        #[clap(long)]
        name: Option<String>,
    },
    /// delete a pending share
    Delete {
        /// ID (full or prefix) of a pending share
        target: String,
    },
}

pub fn share(core: &Core, cmd: ShareCmd) -> Result<(), CliError> {
    match cmd {
        ShareCmd::New { target, username, read_only } => new(core, &target, &username, read_only),
        ShareCmd::Pending { full_ids } => pending(core, full_ids),
        ShareCmd::Accept { target, dest, name } => accept(core, &target, &dest, name),
        ShareCmd::Delete { target } => delete(core, &target),
    }
}

fn new(core: &Core, target: &str, username: &str, read_only: bool) -> Result<(), CliError> {
    let id = resolve_target_to_id(core, target)?;
    let mode = if read_only { lb::ShareMode::Read } else { lb::ShareMode::Write };
    core.share_file(id, username, mode)?;
    println!("done!\nfile '{}' will be shared next time you sync.", id);
    Ok(())
}

fn pending(core: &Core, full_ids: bool) -> Result<(), CliError> {
    let pending_shares = to_share_infos(core.get_pending_shares()?);
    if pending_shares.is_empty() {
        println!("no pending shares.");
        return Ok(());
    }
    print_share_infos(&pending_shares, full_ids);
    Ok(())
}

fn resolve_target_to_pending_share(core: &Core, target: &str) -> Result<lb::File, CliError> {
    let pendings = core.get_pending_shares()?;
    if let Ok(id) = Uuid::parse_str(target) {
        match pendings.iter().find(|f| f.id == id) {
            Some(f) => Ok(f.clone()),
            None => {
                Err(CliError::Console(format!("unable to find pending share with id '{}'", id)))
            }
        }
    } else {
        let possibs: Vec<lb::File> = pendings
            .into_iter()
            .filter(|f| f.id.to_string().starts_with(target))
            .collect();
        match possibs.len() {
            0 => Err(CliError::Console(format!(
                "id prefix '{}' did not match any pending shares",
                target
            ))),
            1 => Ok(possibs[0].clone()),
            n => {
                let mut err_msg =
                    format!("id prefix '{}' matched the following {} pending shares:\n", target, n);
                err_msg += &share_infos_table(&to_share_infos(possibs), true);
                Err(CliError::Console(err_msg))
            }
        }
    }
}

fn accept(
    core: &Core, target: &str, dest: &str, maybe_new_name: Option<String>,
) -> Result<(), CliError> {
    let share = resolve_target_to_pending_share(core, target)?;

    // If a destination ID is provided, it must be of an existing directory.
    let parent_id = if let Ok(id) = Uuid::parse_str(dest) {
        let f = core.get_file_by_id(id)?;
        if !f.is_folder() {
            return Err(CliError::Console(
                "destination ID must be of an existing folder".to_string(),
            ));
        }
        id
    } else {
        // If the destination path exists, it must be a directory. The link will be dropped in it.
        let mut path = dest.to_string();
        if let Some(f) = maybe_get_by_path(core, &path)? {
            if !f.is_folder() {
                return Err(CliError::Console(
                    "existing destination path is a doc, must be a folder".to_string(),
                ));
            }
            f.id
        } else {
            // If the destination path doesn't exist, then it's just treated as a non-existent
            // directory path. The user can set the name with the `--name` input option.
            if !path.ends_with('/') {
                path += "/";
            }
            let f = core.create_at_path(&path)?;
            f.id
        }
    };

    let mut name = maybe_new_name.unwrap_or_else(|| share.name.clone());
    if name.ends_with('/') {
        name.pop(); // Prevent "name contains slash" error.
    }

    core.create_file(&name, parent_id, lb::FileType::Link { target: share.id })
        .map_err(|err| CliError::Console(format!("{:?}", err)))?;
    Ok(())
}

fn delete(core: &Core, target: &str) -> Result<(), CliError> {
    let share = resolve_target_to_pending_share(core, target)?;
    core.delete_pending_share(share.id)?;
    Ok(())
}

struct ShareInfo {
    id: Uuid,
    mode: String,
    name: String,
    from: String,
}

fn to_share_infos(files: Vec<lb::File>) -> Vec<ShareInfo> {
    let mut infos: Vec<ShareInfo> = files
        .into_iter()
        .map(|f| {
            let (from, mode) = f
                .shares
                .get(0)
                .map(|sh| (sh.shared_by.as_str(), sh.mode))
                .unwrap_or(("", lb::ShareMode::Write));
            ShareInfo {
                id: f.id,
                mode: mode.to_string().to_lowercase(),
                name: f.name,
                from: from.to_string(),
            }
        })
        .collect();
    infos.sort_by(|a, b| a.from.cmp(&b.from));
    infos
}

fn print_share_infos(infos: &[ShareInfo], full_ids: bool) {
    println!("{}", share_infos_table(infos, full_ids));
}

fn share_infos_table(infos: &[ShareInfo], full_ids: bool) -> String {
    // Determine each column's max width.
    let w_id = if full_ids { Uuid::nil().to_string().len() } else { ID_PREFIX_LEN };
    let mut w_from = 0;
    let mut w_name = 0;
    let mut w_mode = 0;
    for info in infos {
        let n = info.mode.len();
        if n > w_mode {
            w_mode = n;
        }
        let n = info.from.len();
        if n > w_from {
            w_from = n;
        }
        let n = info.name.len();
        if n > w_name {
            w_name = n;
        }
    }
    // Print the table column headers.
    let mut ret = format!(
        " {:<w_id$} | {:<w_mode$} | {:<w_from$} | file\n",
        "id",
        "mode",
        "from",
        w_id = w_id,
        w_mode = w_mode,
        w_from = w_from
    );
    ret += &format!(
        "-{:-<w_id$}-+-{:-<w_mode$}-+-{:-<w_from$}-+-{:-<w_name$}-\n",
        "",
        "",
        "",
        "",
        w_id = w_id,
        w_mode = w_mode,
        w_from = w_from,
        w_name = w_name
    );
    // Print the table rows of pending share infos.
    for info in infos {
        ret += &format!(
            " {:<w_id$} | {:<w_mode$} | {:<w_from$} | {}\n",
            &info.id.to_string()[..w_id],
            info.mode,
            info.from,
            info.name,
            w_id = w_id,
            w_mode = w_mode,
            w_from = w_from
        );
    }
    ret
}
