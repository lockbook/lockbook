use crate::Github;

mod cli;

pub fn release_linux(version: Option<&str>) {
    cli::release(&Github::env(), version);
}
