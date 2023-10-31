use crate::ensure_account_and_root;
use crate::input::FileInput;
use cli_rs::cli_error::CliResult;
use lb::Core;
use std::io;
use std::io::{Read, Write};

pub fn stdin(core: &Core, target: FileInput, append: bool) -> CliResult<()> {
    ensure_account_and_root(core)?;
    let id = target.find(core)?.id;

    let mut stdin = io::stdin().lock();
    let mut buffer = [0; 512];
    let mut document = if append { core.read_document(id)? } else { vec![] };

    loop {
        let bytes = stdin.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        document.extend_from_slice(&buffer[..bytes]);
        core.write_document(id, &document)?;
    }
    Ok(())
}

pub fn stdout(core: &Core, target: FileInput) -> CliResult<()> {
    ensure_account_and_root(core)?;

    let id = target.find(core)?.id;
    let content = core.read_document(id)?;
    print!("{}", String::from_utf8_lossy(&content));
    io::stdout().flush()?;
    Ok(())
}
