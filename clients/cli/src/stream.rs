use crate::input::find_file;
use crate::{core, ensure_account_and_root};
use cli_rs::cli_error::{CliError, CliResult};
use lb_rs::model::errors::LbErrKind;
use lb_rs::{Lb, Uuid};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::{fs, io};

#[tokio::main]
pub async fn stdin(target: String, append: bool, base: Option<PathBuf>) -> CliResult<()> {
    if let Some(base) = &base {
        if base.as_os_str().is_empty() {
            return Err(CliError::from("--base needs the file you read into, e.g. --base=read.md"));
        }
        if append {
            return Err(CliError::from("--append and --base can't be used together"));
        }
    }
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    if let Some(base) = base {
        let base = fs::read(&base)
            .map_err(|err| CliError::from(format!("couldn't read {}: {err}", base.display())))?;
        let mut ours = vec![];
        io::stdin().lock().read_to_end(&mut ours)?;
        let id = find_file(lb, &target).await?.id;
        if write_merging(lb, id, &base, ours).await? {
            eprintln!("merged with changes made since the read");
        }
    } else {
        let id = resolve_or_create(lb, &target).await?;
        write_streaming(lb, id, append).await?;
    }
    Ok(())
}

async fn write_streaming(lb: &Lb, id: Uuid, append: bool) -> CliResult<()> {
    let mut stdin = io::stdin().lock();
    let mut buffer = [0; 512];
    let mut document = if append { lb.read_document(id, true).await? } else { vec![] };

    loop {
        let bytes = stdin.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        document.extend_from_slice(&buffer[..bytes]);
        lb.write_document(id, &document).await?;
    }
    Ok(())
}

/// Writes as [Lb::write_from_base] does, refusing with a message to read again. Returns whether
/// there were changes to merge.
pub async fn write_merging(lb: &Lb, id: Uuid, base: &[u8], ours: Vec<u8>) -> CliResult<bool> {
    match lb.write_from_base(id, base, ours, None).await {
        Ok((_, merged)) => Ok(merged),
        Err(err) if err.kind == LbErrKind::ReReadRequired => {
            let name = lb.get_file_by_id(id).await?.name;
            Err(CliError::from(format!(
                "{name} changed since it was read and the changes can't be merged; \
                 nothing was written, read it again and redo the edit"
            )))
        }
        Err(err) => Err(err.into()),
    }
}

async fn resolve_or_create(lb: &Lb, target: &str) -> CliResult<Uuid> {
    if let Ok(f) = find_file(lb, target).await {
        return Ok(f.id);
    }
    if Uuid::from_str(target).is_ok() {
        return Err(CliError::from("cannot create a file using ids"));
    }
    Ok(lb.create_at_path(target).await?.id)
}

#[tokio::main]
pub async fn stdout(target: String) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let id = find_file(lb, &target).await?.id;
    let content = lb.read_document(id, true).await?;
    // the bytes as stored, so a copy saved from here can be a --base
    let mut out = io::stdout().lock();
    match out.write_all(&content).and_then(|()| out.flush()) {
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => std::process::exit(0),
        result => Ok(result?),
    }
}
