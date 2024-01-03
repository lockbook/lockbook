use crate::utils::CommandRunner;
use std::process::Command;

pub fn build() {
    Command::new("bash")
        .args(["create_libs.sh"])
        .current_dir("libs/content/workspace-ffi/SwiftWorkspace/")
        .assert_success();
}
