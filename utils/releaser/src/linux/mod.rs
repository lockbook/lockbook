use crate::{utils::edit_cargo_version, Github};

mod cli;

pub fn release_linux(version: &str) {
    edit_cargo_version("clients/cli", version);
    cli::release(&Github::env(), version);
}
