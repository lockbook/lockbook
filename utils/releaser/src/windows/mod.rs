mod cli;
mod egui;

use std::fs;
use std::path::Path;

use crate::Github;

pub fn release(gh: &Github, version: Option<&str>) {
    let build_dir = Path::new("windows-build");
    if !build_dir.exists() {
        fs::create_dir("windows-build").unwrap();
    }
    cli::release(gh);
    egui::release_installers(gh);

    fs::remove_dir_all("windows-build").unwrap();
}
