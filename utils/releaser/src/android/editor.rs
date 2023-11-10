use crate::utils::CommandRunner;
use std::process::Command;

pub fn build() {
    Command::new("bash")
        .args(["create_android_libs.sh"])
        .current_dir("libs/content/editor/egui_editor")
        .assert_success();
}
