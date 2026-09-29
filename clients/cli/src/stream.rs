use crate::input::find_file;
use crate::{core, ensure_account_and_root};
use cli_rs::cli_error::{CliError, CliResult};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::text::buffer::Buffer as TextBuffer;
use lb_rs::{Lb, Uuid};
use std::io;
use std::io::{Read, Write};
use std::str::FromStr;

#[tokio::main]
pub async fn stdin(target: String, append: bool) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;
    let id = resolve_or_create(lb, &target).await?;

    let (base_hmac, base_bytes) = lb.read_document_with_hmac(id, true).await?;
    let mut ours_bytes = if append { base_bytes.clone() } else { Vec::new() };
    io::stdin().lock().read_to_end(&mut ours_bytes)?;

    let base_text = String::from_utf8_lossy(&base_bytes);
    let ours_text = String::from_utf8_lossy(&ours_bytes);

    loop {
        let (disk_hmac, disk_bytes) = lb.read_document_with_hmac(id, false).await?;
        let to_write = if disk_hmac == base_hmac {
            ours_bytes.clone()
        } else {
            let theirs = String::from_utf8_lossy(&disk_bytes);
            TextBuffer::from(base_text.as_ref())
                .merge(ours_text.clone().into_owned(), theirs.into_owned())
                .into_bytes()
        };

        match lb.safe_write(id, disk_hmac, to_write, None).await {
            Ok(_) => {
                if disk_hmac != base_hmac {
                    eprintln!("Merged concurrent changes into the document.");
                }
                return Ok(());
            }
            Err(e) if matches!(e.kind, LbErrKind::ReReadRequired) => continue,
            Err(e) => return Err(CliError::from(e.to_string())),
        }
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
    print!("{}", String::from_utf8_lossy(&content));
    io::stdout().flush()?;
    Ok(())
}
