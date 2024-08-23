use cli_rs::cli_error::{CliError, CliResult};
use lb::{Core, Uuid};

use crate::{ensure_account_and_root, input::FileInput};

pub fn new(core: &Core, target: FileInput, username: String, read_only: bool) -> CliResult<()> {
    ensure_account_and_root(core)?;

    let id = target.find(core)?.id;
    let mode = if read_only { lb::ShareMode::Read } else { lb::ShareMode::Write };
    core.share_file(id, &username, mode)?;
    println!("done!\nfile '{}' will be shared next time you sync.", id);
    Ok(())
}

pub fn pending(core: &Core) -> CliResult<()> {
    ensure_account_and_root(core)?;

    let pending_shares = to_share_infos(core.get_pending_shares()?);
    if pending_shares.is_empty() {
        println!("no pending shares.");
        return Ok(());
    }
    print_share_infos(&pending_shares);
    Ok(())
}

pub fn accept(core: &Core, target: Uuid, dest: FileInput) -> CliResult<()> {
    ensure_account_and_root(core)?;

    let share = core
        .get_pending_shares()?
        .into_iter()
        .find(|f| f.id == target)
        .ok_or_else(|| CliError::from(format!("Could not find {target} in pending shares")))?;
    let parent = dest.find(core)?;

    core.create_file(&share.name, parent.id, lb::FileType::Link { target: share.id })
        .map_err(|err| CliError::from(format!("{:?}", err)))?;

    Ok(())
}

pub fn delete(core: &Core, target: Uuid) -> Result<(), CliError> {
    ensure_account_and_root(core)?;

    let share = core
        .get_pending_shares()?
        .into_iter()
        .find(|f| f.id == target)
        .ok_or_else(|| CliError::from(format!("Could not find {target} in pending shares")))?;
    core.delete_pending_share(share.id)?;
    Ok(())
}

fn print_share_infos(infos: &[ShareInfo]) {
    println!("{}", share_infos_table(infos));
}

fn to_share_infos(files: Vec<lb::File>) -> Vec<ShareInfo> {
    let mut infos: Vec<ShareInfo> = files
        .into_iter()
        .map(|f| {
            let (from, mode) = f
                .shares
                .first()
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

struct ShareInfo {
    id: Uuid,
    mode: String,
    name: String,
    from: String,
}

fn share_infos_table(infos: &[ShareInfo]) -> String {
    // Determine each column's max width.
    let w_id = Uuid::nil().to_string().len();
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

pub fn pending_share_completor(
    core: &lb::CoreLib<lb::service::api_service::NetworkOld, lb::OnDiskDocuments>, prompt: &str,
) -> Result<Vec<String>, CliError> {
    Ok(core
        .get_pending_shares()?
        .into_iter()
        .map(|share| share.id.to_string())
        .filter(|id| id.starts_with(prompt))
        .collect())
}
