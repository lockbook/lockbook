use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(apple_ai_native)");
    println!("cargo:rerun-if-changed=src/bridge.swift");
    println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");
    println!("cargo:rerun-if-env-changed=LB_APPLE_AI_DISABLE");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("aarch64")
        || env::var_os("LB_APPLE_AI_DISABLE").is_some()
    {
        return;
    }
    let sdk = xcrun(&["--sdk", "macosx", "--show-sdk-path"]);
    let version = xcrun(&["--sdk", "macosx", "--show-sdk-version"]);
    if version
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0)
        < 26
    {
        println!(
            "cargo:warning=Apple Intelligence disabled: build with Xcode 26 or newer to enable it"
        );
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut compiler = Command::new("xcrun");
    compiler
        .args([
            "swiftc",
            "-parse-as-library",
            "-emit-library",
            "-static",
            "-swift-version",
            "6",
            "-module-name",
            "LockbookAppleAI",
            "-target",
            "arm64-apple-macosx14.0",
            "-sdk",
            &sdk,
            "-O",
            "src/bridge.swift",
            "-o",
        ])
        .arg(out.join("liblb_apple_ai_bridge.a"));
    let parts: Vec<u32> = version.split('.').filter_map(|v| v.parse().ok()).collect();
    if parts[0] > 26 || parts.get(1).copied().unwrap_or(0) >= 4 {
        compiler.args(["-D", "LB_APPLE_TOKEN_COUNT"]);
    }
    let status = compiler.status().expect("run Swift compiler");
    assert!(status.success(), "Apple Intelligence Swift bridge failed to compile");
    println!("cargo:rustc-cfg=apple_ai_native");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=lb_apple_ai_bridge");
    // Examples/tests need the system Swift runtime's rpath. Downstream Rust
    // executables supply it too; Xcode supplies it for Swift application targets.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    let swift = PathBuf::from(xcrun(&["--find", "swiftc"]));
    let runtime = swift
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("lib/swift/macosx");
    println!("cargo:rustc-link-search=native={}", runtime.display());
    // Swift emits weak framework autolinks because the bridge targets macOS 14.
    // Do not add a strong FoundationModels link: that would break older systems.
}

fn xcrun(args: &[&str]) -> String {
    Command::new("xcrun")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}
