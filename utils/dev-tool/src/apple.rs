use crate::utils::HashInfo;
use crate::{utils, CliError, ToolEnvironment};
use execute_command_macro::{command, command_args};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const LIB_NAME_HEADER: &str = "lockbook_core.h";
const LIB_NAME: &str = "liblockbook_core.a";

pub fn run_swift_tests(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let hash_info = HashInfo::get_from_disk(&tool_env.commit_hash)?;
    dotenv::from_path(utils::test_env_path(&tool_env.root_dir))?;

    make_swift_test_lib(tool_env.clone())?;

    let build_results = command!("swift build")
        .current_dir(utils::swift_dir(&tool_env.root_dir))
        .spawn()?
        .wait()?;

    if !build_results.success() {
        return Err(CliError::basic_error());
    }

    let test_results = command!("swift test --generate-linuxmain")
        .current_dir(utils::swift_dir(&tool_env.root_dir))
        .env("API_URL", utils::get_api_url(hash_info.get_port()?))
        .spawn()?
        .wait()?;

    if !test_results.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}

pub fn make_swift_test_lib(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let core_dir = utils::core_dir(&tool_env.root_dir);
    let c_interface_dir = core_dir
        .join("src/external_interface/c_interface.rs")
        .to_str()
        .ok_or(CliError(Some("Couldn't get c interface file dir.".to_string())))?
        .to_string();

    let build_results = Command::new("cbindgen")
        .args([&c_interface_dir, "-l", "c"])
        .current_dir(utils::core_dir(&tool_env.root_dir))
        .stdout(Stdio::piped())
        .output()?;

    if !build_results.status.success() {
        return Err(CliError::basic_error());
    }

    let swift_inc_dir = utils::swift_inc(&tool_env.root_dir);

    println!("{}", swift_inc_dir.join(LIB_NAME_HEADER).to_str().unwrap());

    fs::create_dir_all(&swift_inc_dir)?;
    File::create(swift_inc_dir.join(LIB_NAME_HEADER))?
        .write_all(build_results.stdout.as_slice())?;

    let build_results = command!("cargo build --release")
        .current_dir(utils::core_dir(&tool_env.root_dir))
        .spawn()?
        .wait()?;

    if !build_results.success() {
        return Err(CliError::basic_error());
    }

    let swift_lib_dir = utils::swift_lib(&tool_env.root_dir);

    fs::create_dir_all(&swift_lib_dir)?;

    fs::copy(tool_env.target_dir.join("release").join(LIB_NAME), swift_lib_dir.join(LIB_NAME))?;

    Ok(())
}
