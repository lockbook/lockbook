use crate::core;
use cli_rs::cli_error::{CliError, CliResult};
use lb_rs::Uuid;
use lb_rs::model::file::{File, ShareMode};
use lb_rs::model::file_metadata::FileType;

use crate::ensure_account_and_root;
use crate::input::FileInput;

#[tokio::main]
pub async fn new(target: FileInput, username: String, read_only: bool) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let id = target.find(lb).await?.id;
    let mode = if read_only { ShareMode::Read } else { ShareMode::Write };
    lb.share_file(id, &username, mode).await?;
    println!("done!\nfile '{id}' will be shared next time you sync.");
    Ok(())
}

#[tokio::main]
pub async fn pending() -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let pending_shares = to_share_infos(lb.get_pending_shares().await?);
    if pending_shares.is_empty() {
        println!("no pending shares.");
        return Ok(());
    }
    print_share_infos(&pending_shares);
    Ok(())
}

#[tokio::main]
pub async fn accept(target: &Uuid, dest: FileInput) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let share = lb
        .get_pending_shares()
        .await?
        .into_iter()
        .find(|f| f.id == *target)
        .ok_or_else(|| CliError::from(format!("Could not find {target} in pending shares")))?;
    let parent = dest.find(lb).await?;

    lb.create_file(&share.name, &parent.id, FileType::Link { target: share.id })
        .await
        .map_err(|err| CliError::from(format!("{err:?}")))?;

    Ok(())
}

#[tokio::main]
pub async fn delete(target: Uuid) -> Result<(), CliError> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let share = lb
        .get_pending_shares()
        .await?
        .into_iter()
        .find(|f| f.id == target)
        .ok_or_else(|| CliError::from(format!("Could not find {target} in pending shares")))?;
    lb.reject_share(&share.id).await?;
    Ok(())
}

fn print_share_infos(infos: &[ShareInfo]) {
    println!("{}", share_infos_table(infos));
}

fn to_share_infos(files: Vec<File>) -> Vec<ShareInfo> {
    let mut infos: Vec<ShareInfo> = files
        .into_iter()
        .map(|f| {
            let (from, mode) = f
                .shares
                .first()
                .map(|sh| (sh.shared_by.as_str(), sh.mode))
                .unwrap_or(("", ShareMode::Write));
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

#[tokio::main]
pub async fn pending_share_completor(prompt: &str) -> CliResult<Vec<String>> {
    let lb = &core().await?;
    Ok(lb
        .get_pending_shares()
        .await?
        .into_iter()
        .map(|share| share.id.to_string())
        .filter(|id| id.starts_with(prompt))
        .collect())
}
