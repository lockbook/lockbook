use crate::input::FileInput;
use crate::{core, ensure_account_and_root};
use cli_rs::cli_error::CliResult;
use std::io;
use std::io::{Read, Write};

#[tokio::main]
pub async fn stdin(target: FileInput, append: bool) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;
    let id = target.find(lb).await?.id;

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

#[tokio::main]
pub async fn stdout(target: FileInput) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let id = target.find(lb).await?.id;
    let content = lb.read_document(id, true).await?;
    print!("{}", String::from_utf8_lossy(&content));
    io::stdout().flush()?;
    Ok(())
}
