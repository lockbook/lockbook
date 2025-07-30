use crate::ToolEnvironment;
use crate::utils::{self, CommandRunner};

use std::fs;
use std::path::Path;
use std::process::Command;

const MIN_NDK_VERSION: u32 = 22;

#[cfg(target_os = "macos")]
const NDK_TEST_LIB_NAME: &str = "liblb_external_interface.dylib";
#[cfg(not(target_os = "macos"))]
const NDK_TEST_LIB_NAME: &str = "liblb_external_interface.so";

const NDK_ANDROID_LIB_NAME: &str = "liblb_external_interface.so";

pub fn fmt_android(tool_env: &ToolEnvironment) {
    let android_dir = utils::android_dir(&tool_env.root_dir);

    Command::new(android_dir.join("gradlew"))
        .arg("lintKotlin")
        .current_dir(android_dir)
        .assert_success();
}

pub fn lint_android(tool_env: &ToolEnvironment) {
    let android_dir = utils::android_dir(&tool_env.root_dir);

    Command::new(android_dir.join("gradlew"))
        .arg("lint")
        .current_dir(android_dir)
        .assert_success();
}

pub fn run_kotlin_tests(tool_env: &ToolEnvironment) {
    dotenvy::from_path(utils::local_env_path(&tool_env.root_dir)).unwrap();

    make_android_test_lib(tool_env);

    let android_dir = utils::android_dir(&tool_env.root_dir);

    Command::new(android_dir.join("gradlew"))
        .arg("testDebugUnitTest")
        .current_dir(utils::android_dir(&tool_env.root_dir))
        .assert_success();
}

pub fn make_android_libs(tool_env: &ToolEnvironment) {
    let ext_iface_dir = utils::lb_external_interface_dir(&tool_env.root_dir);

    build_core_for_android_arch(&ext_iface_dir, "aarch64-linux-android");
    build_core_for_android_arch(&ext_iface_dir, "armv7-linux-androideabi");
    build_core_for_android_arch(&ext_iface_dir, "i686-linux-android");
    build_core_for_android_arch(&ext_iface_dir, "x86_64-linux-android");

    let jni_lib_dir = utils::jni_lib_dir(&tool_env.root_dir);

    fs::create_dir_all(jni_lib_dir.join("arm64-v8a")).unwrap();
    fs::create_dir_all(jni_lib_dir.join("armeabi-v7a")).unwrap();
    fs::create_dir_all(jni_lib_dir.join("x86")).unwrap();
    fs::create_dir_all(jni_lib_dir.join("x86_64")).unwrap();

    fs::copy(
        tool_env
            .target_dir
            .join("aarch64-linux-android/release")
            .join(NDK_ANDROID_LIB_NAME),
        jni_lib_dir.join("arm64-v8a").join(NDK_ANDROID_LIB_NAME),
    )
    .unwrap();
    fs::copy(
        tool_env
            .target_dir
            .join("armv7-linux-androideabi/release")
            .join(NDK_ANDROID_LIB_NAME),
        jni_lib_dir.join("armeabi-v7a").join(NDK_ANDROID_LIB_NAME),
    )
    .unwrap();
    fs::copy(
        tool_env
            .target_dir
            .join("i686-linux-android/release")
            .join(NDK_ANDROID_LIB_NAME),
        jni_lib_dir.join("x86").join(NDK_ANDROID_LIB_NAME),
    )
    .unwrap();
    fs::copy(
        tool_env
            .target_dir
            .join("x86_64-linux-android/release")
            .join(NDK_ANDROID_LIB_NAME),
        jni_lib_dir.join("x86_64").join(NDK_ANDROID_LIB_NAME),
    )
    .unwrap();
}

pub fn make_android_test_lib(tool_env: &ToolEnvironment) {
    Command::new("cargo")
        .args(["build", "--lib", "--release"])
        .current_dir(utils::lb_external_interface_dir(&tool_env.root_dir))
        .assert_success();

    let jni_lib_dir = utils::jni_lib_dir(&tool_env.root_dir);

    fs::create_dir_all(jni_lib_dir.join("desktop")).unwrap();

    fs::copy(
        tool_env.target_dir.join("release").join(NDK_TEST_LIB_NAME),
        jni_lib_dir.join("desktop").join(NDK_TEST_LIB_NAME),
    )
    .unwrap();
}

fn build_core_for_android_arch<P: AsRef<Path>>(core_dir: P, platform: &str) {
    Command::new("cargo")
        .args([
            "ndk",
            "--target",
            platform,
            "--platform",
            &MIN_NDK_VERSION.to_string(),
            "--",
            "build",
            "--release",
        ])
        .current_dir(core_dir)
        .assert_success();
}
