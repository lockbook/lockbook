use crate::Github;

mod cli;
mod desktop;

pub fn release_linux() {
    cli::release(&Github::env());
    desktop::release(&Github::env());
}
