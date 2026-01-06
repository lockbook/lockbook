#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod input;
#[cfg(windows)]
mod output;
#[cfg(windows)]
mod window;

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    window::main().unwrap();
}
