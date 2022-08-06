use crate::utils::HashInfo;
use crate::{utils, CliError, ToolEnvironment};
use execute_command_macro::{command, command_args};
use std::fs;
use std::path::Path;

const MIN_NDK_VERSION: u32 = 22;
const NDK_LIB_NAME: &str = "liblockbook_core.so";

pub fn fmt_android(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let fmt_result = command!("./gradlew lintKotlin")
        .current_dir(utils::android_dir(tool_env.root_dir))
        .spawn()?
        .wait()?;

    if !fmt_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}

pub fn lint_android(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let lint_result = command!("./gradlew lint")
        .current_dir(utils::android_dir(tool_env.root_dir))
        .spawn()?
        .wait()?;

    if !lint_result.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}

pub fn run_kotlin_tests(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let hash_info = HashInfo::get_from_disk(&tool_env.commit_hash)?;
    dotenv::from_path(utils::test_env_path(&tool_env.root_dir))?;

    make_android_test_lib(tool_env.clone())?;

    let test_results = command!("./gradlew testDebugUnitTest")
        .current_dir(utils::android_dir(&tool_env.root_dir))
        .env("API_URL", utils::get_api_url(hash_info.get_port()?))
        .spawn()?
        .wait()?;

    if !test_results.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}

pub fn make_android_libs(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let core_dir = utils::core_dir(&tool_env.root_dir);

    build_core_for_android_arch(&core_dir, "aarch64-linux-android")?;
    build_core_for_android_arch(&core_dir, "armv7-linux-androideabi")?;
    build_core_for_android_arch(&core_dir, "i686-linux-android")?;
    build_core_for_android_arch(&core_dir, "x86_64-linux-android")?;

    let jni_lib_dir = utils::jni_lib_dir(&tool_env.root_dir);

    fs::create_dir_all(jni_lib_dir.join("arm64-v8a"))?;
    fs::create_dir_all(jni_lib_dir.join("armeabi-v7a"))?;
    fs::create_dir_all(jni_lib_dir.join("x86"))?;
    fs::create_dir_all(jni_lib_dir.join("x86_64"))?;

    fs::rename(
        tool_env
            .target_dir
            .join(format!("aarch64-linux-android/release/{}", NDK_LIB_NAME)),
        jni_lib_dir.join(format!("arm64-v8a/{}", NDK_LIB_NAME)),
    )?;
    fs::rename(
        tool_env
            .target_dir
            .join(format!("armv7-linux-androideabi/release/{}", NDK_LIB_NAME)),
        jni_lib_dir.join(format!("armeabi-v7a/{}", NDK_LIB_NAME)),
    )?;
    fs::rename(
        tool_env
            .target_dir
            .join(format!("i686-linux-android/release/{}", NDK_LIB_NAME)),
        jni_lib_dir.join(format!("x86/{}", NDK_LIB_NAME)),
    )?;
    fs::rename(
        tool_env
            .target_dir
            .join(format!("x86_64-linux-android/release/{}", NDK_LIB_NAME)),
        jni_lib_dir.join(format!("x86_64/{}", NDK_LIB_NAME)),
    )?;

    Ok(())
}

pub fn make_android_test_lib(tool_env: ToolEnvironment) -> Result<(), CliError> {
    let core_dir = utils::core_dir(&tool_env.root_dir);

    let test_results = command!("cargo build --lib --release")
        .current_dir(&tool_env.root_dir)
        .spawn()?
        .wait()?;

    let jni_lib_dir = utils::jni_lib_dir(&tool_env.root_dir);

    fs::create_dir_all(jni_lib_dir.join("desktop"))?;

    fs::rename(
        tool_env
            .target_dir
            .join(format!("release/{}", NDK_LIB_NAME)),
        jni_lib_dir.join(format!("desktop/{}", NDK_LIB_NAME)),
    )?;

    Ok(())
}

fn build_core_for_android_arch<P: AsRef<Path>>(
    core_dir: P, platform: &str,
) -> Result<(), CliError> {
    let build_results = command_args!(
        "cargo",
        "ndk",
        "--target",
        platform,
        "--platform",
        MIN_NDK_VERSION.to_string(),
        "--",
        "build",
        "--release"
    )
    .current_dir(core_dir)
    .spawn()?
    .wait()?;

    if !build_results.success() {
        return Err(CliError::basic_error());
    }

    Ok(())
}
