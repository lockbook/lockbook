use crate::utils::CommandRunner;
use std::fs;
use std::process::Command;

const MIN_NDK_VERSION: u32 = 22;
const NDK_LIB_NAME: &str = "liblb_external_interface.so";
const JNI_LIB: &str = "clients/android/core/src/main/jniLibs";

const ARCH64: &str = "aarch64-linux-android";
const ARMV7: &str = "armv7-linux-androideabi";
const I686: &str = "i686-linux-android";
const X86_64: &str = "x86_64-linux-android";

const SHORT_ARCH64: &str = "arm64-v8a";
const SHORT_ARMV7: &str = "armeabi-v7a";
const SHORT_I686: &str = "x86";
const SHORT_X86_64: &str = "x86_64";

pub fn build_libs() {
    build_lb_for_android_arch(ARCH64);
    build_lb_for_android_arch(ARMV7);
    build_lb_for_android_arch(I686);
    build_lb_for_android_arch(X86_64);

    let android_arch64 = format!("{JNI_LIB}/{SHORT_ARCH64}");
    let android_armv7 = format!("{JNI_LIB}/{SHORT_ARMV7}");
    let android_i686 = format!("{JNI_LIB}/{SHORT_I686}");
    let android_x86_64 = format!("{JNI_LIB}/{SHORT_X86_64}");

    fs::create_dir_all(&android_arch64).unwrap();
    fs::create_dir_all(&android_armv7).unwrap();
    fs::create_dir_all(&android_i686).unwrap();
    fs::create_dir_all(&android_x86_64).unwrap();

    fs::copy(
        format!("target/{ARCH64}/release/{NDK_LIB_NAME}"),
        format!("{android_arch64}/{NDK_LIB_NAME}"),
    )
    .unwrap();
    fs::copy(
        format!("target/{ARMV7}/release/{NDK_LIB_NAME}"),
        format!("{android_armv7}/{NDK_LIB_NAME}"),
    )
    .unwrap();
    fs::copy(
        format!("target/{I686}/release/{NDK_LIB_NAME}"),
        format!("{android_i686}/{NDK_LIB_NAME}"),
    )
    .unwrap();
    fs::copy(
        format!("target/{X86_64}/release/{NDK_LIB_NAME}"),
        format!("{android_x86_64}/{NDK_LIB_NAME}"),
    )
    .unwrap();
}

fn build_lb_for_android_arch(platform: &str) {
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
        .current_dir("libs/lb_external_interface")
        .assert_success();
}
