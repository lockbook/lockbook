use crate::Github;

mod cli;

pub fn release_linux() {
    cli::release(&Github::env());
}
