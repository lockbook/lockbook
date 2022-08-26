mod cli;
mod egui;

use std::path::Path;

use crate::Github;

pub fn release(gh: &Github) {
    cli::release(gh);
    egui::release_installers(gh);
    clean_build_dir();
}

fn clean_build_dir() {
    let build_dir = Path::new("windows-build");
    if build_dir.exists() {
        std::fs::remove_dir_all("windows-build").unwrap();
    }
}
