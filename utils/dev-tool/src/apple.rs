use crate::ToolEnvironment;
use crate::utils::{self, CommandRunner};
use std::fs::{self, File};
use std::io::Write;
use std::process::{Command, Stdio};

const LIB_NAME_HEADER: &str = "lb_rs.h";
const LIB_NAME: &str = "liblb_external_interface.a";

pub fn run_swift_tests(tool_env: &ToolEnvironment) {
    dotenvy::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();

    make_swift_test_lib(tool_env);

    let swift_core_dir = utils::swift_core_dir(&tool_env.root_dir);

    Command::new("swift")
        .arg("build")
        .current_dir(&swift_core_dir)
        .assert_success();

    Command::new("swift")
        .arg("test")
        .current_dir(&swift_core_dir)
        .assert_success();
}

pub fn make_swift_test_lib(tool_env: &ToolEnvironment) {
    let ext_iface_dir = utils::lb_external_interface_dir(&tool_env.root_dir);
    let c_interface_file = ext_iface_dir
        .join("src/c_interface.rs")
        .to_str()
        .unwrap()
        .to_string();

    let build_results = Command::new("cbindgen")
        .args([&c_interface_file, "-l", "c"])
        .current_dir(&ext_iface_dir)
        .stdout(Stdio::piped())
        .assert_success_with_output();

    let swift_inc_dir = utils::swift_inc(&tool_env.root_dir);

    fs::create_dir_all(&swift_inc_dir).unwrap();
    File::create(swift_inc_dir.join(LIB_NAME_HEADER))
        .unwrap()
        .write_all(build_results.stdout.as_slice())
        .unwrap();

    Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&ext_iface_dir)
        .assert_success();

    let swift_lib_dir = utils::swift_lib(&tool_env.root_dir);

    fs::create_dir_all(&swift_lib_dir).unwrap();

    fs::copy(tool_env.target_dir.join("release").join(LIB_NAME), swift_lib_dir.join(LIB_NAME))
        .unwrap();
}
