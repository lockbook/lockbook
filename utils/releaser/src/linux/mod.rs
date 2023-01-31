use crate::Github;

mod cli;

pub fn release_linux(version: &str) {
    cli::release(&Github::env(), version);
}
