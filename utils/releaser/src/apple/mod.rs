mod cli;
mod core;
mod editor;
mod ios;
mod mac;

use crate::secrets::AppStore;
use crate::Github;
use std::fs;
use std::path::Path;

pub fn release_apple(gh: &Github, asc: &AppStore, version: Option<&str>) {
    cli::release(gh);
    core::build();
    editor::build();
    clean_build_dir();
    ios::release(asc);
    mac::release(asc, gh);
    todo!("upgrade version to {}", version.unwrap());
}

fn clean_build_dir() {
    let build_dir = Path::new("clients/apple/build");
    if build_dir.exists() {
        fs::remove_dir_all("clients/apple/build").unwrap()
    }
}
