use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use cli_rs::cli_error::{CliError, CliResult};

use crate::{
    places::{
        android_dir, apple_dir, local_env_path, root, target, workspace_ffi, workspace_swift_libs,
    },
    utils::CommandRunner,
};

pub fn apple_ws_all() -> CliResult<()> {
    apple_ws(WsBuildTargets { ios: true, ios_sim: true, arm_macos: true, x86_macos: true })
}

pub fn apple_ws_macos() -> CliResult<()> {
    apple_ws(WsBuildTargets { ios: false, ios_sim: false, arm_macos: true, x86_macos: true })?;
    println!(
        "warning: xcode may need to be restarted if you swap between iOS & macOS and experience build failures"
    );
    Ok(())
}

pub fn apple_ws_ios() -> CliResult<()> {
    apple_ws(WsBuildTargets { ios: true, ios_sim: false, arm_macos: false, x86_macos: false })?;
    println!(
        "warning: xcode may need to be restarted if you swap between iOS & macOS and experience build failures"
    );
    Ok(())
}

#[derive(Copy, Clone)]
pub struct WsBuildTargets {
    ios: bool,
    ios_sim: bool,
    arm_macos: bool,
    x86_macos: bool,
}

pub fn android_ws() -> CliResult<()> {
    Command::new("libs/content/workspace-ffi/create_android_libs.sh")
        .current_dir(root())
        .assert_success()?;

    Ok(())
}

fn apple_ws(targets: WsBuildTargets) -> CliResult<()> {
    println!("cbindgen");
    let _ = fs::remove_dir_all(workspace_swift_libs());
    fs::create_dir_all(workspace_ffi().join("include"))?;
    Command::new("cbindgen")
        .args(["-l", "c", "-o", "include/workspace.h"])
        .current_dir(workspace_ffi())
        .assert_success()?;

    let mut ios_build = Command::new("cargo");
    let mut args = vec!["build", "--release"];
    let mut execute_ios = false;

    if targets.ios {
        execute_ios = true;
        args.push("--target=aarch64-apple-ios");
    }

    if targets.ios_sim {
        execute_ios = true;
        args.push("--target=aarch64-apple-ios-sim");
    }

    if execute_ios {
        println!("Building iOS");
        ios_build
            .env("IPHONEOS_DEPLOYMENT_TARGET", "26.0")
            .args(args)
            .current_dir(workspace_ffi())
            .assert_success()?;
    }

    let mut mac_build = Command::new("cargo");
    let mut args = vec!["build", "--release"];
    let mut execute_mac = false;

    if targets.arm_macos {
        execute_mac = true;
        args.push("--target=aarch64-apple-darwin");
    }

    if targets.x86_macos {
        execute_mac = true;
        args.push("--target=x86_64-apple-darwin");
    }

    if execute_mac {
        println!("Building macOS");
        mac_build
            .env("MACOSX_DEPLOYMENT_TARGET", "26.0")
            .args(args)
            .current_dir(workspace_ffi())
            .assert_success()?;
    }

    fs::create_dir_all(workspace_swift_libs())?;
    if targets.arm_macos && targets.x86_macos {
        println!("lipoing");
        Command::new("lipo")
            .args([
                "-create",
                "target/aarch64-apple-darwin/release/libworkspace.a",
                "target/x86_64-apple-darwin/release/libworkspace.a",
                "-output",
                workspace_swift_libs()
                    .join("libworkspace.a")
                    .to_str()
                    .unwrap(),
            ])
            .current_dir(root())
            .assert_success()?;
    } else if targets.arm_macos {
        fs::rename(
            target().join("aarch64-apple-darwin/release/libworkspace.a"),
            workspace_swift_libs().join("libworkspace.a"),
        )?;
    } else if targets.x86_macos {
        fs::rename(
            target().join("x86_64-apple-darwin/release/libworkspace.a"),
            workspace_swift_libs().join("libworkspace.a"),
        )?;
    }

    println!("building xcframework");
    let mut xcframework = Command::new("xcodebuild");

    let mut args = vec!["-create-xcframework"];

    if targets.arm_macos || targets.x86_macos {
        args.push("-library");
        args.push("libworkspace.a");
        args.push("-headers");
        args.push("../../include");
    }

    if targets.ios {
        args.push("-library");
        args.push("../../../../../target/aarch64-apple-ios/release/libworkspace.a");
        args.push("-headers");
        args.push("../../include");
    }

    if targets.ios_sim {
        args.push("-library");
        args.push("../../../../../target/aarch64-apple-ios-sim/release/libworkspace.a");
        args.push("-headers");
        args.push("../../include");
    }

    args.push("-output");
    args.push("workspace.xcframework");

    xcframework
        .current_dir(workspace_swift_libs())
        .args(args)
        .assert_success()?;

    Ok(())
}

pub fn apple_run_ios(name: String) -> CliResult<()> {
    let device_id = get_device_id(&name)?;
    apple_ws_ios()?;
    Command::new("xcodebuild")
        .args([
            "-workspace",
            "clients/apple/lockbook.xcworkspace",
            "-scheme",
            "Lockbook",
            "-destination",
            "generic/platform=iOS",
            "-configuration",
            "Release",
            "-archivePath",
            "clients/apple/build/Lockbook-iOS.xcarchive",
            "archive",
        ])
        .current_dir(root())
        .assert_success()?;

    Command::new("xcrun")
        .args([
            "devicectl",
            "device",
            "install",
            "app",
            "--device",
            &device_id,
            "build/Lockbook-iOS.xcarchive/Products/Applications/Lockbook.app/",
        ])
        .current_dir(apple_dir())
        .assert_success()?;

    Command::new("xcrun")
        .args([
            "devicectl",
            "device",
            "process",
            "launch",
            "--console",
            "--device",
            &device_id,
            "app.lockbook",
        ])
        .assert_success()?;
    Ok(())
}
pub fn apple_run_macos() -> CliResult<()> {
    apple_ws_macos()?;

    Command::new("xcodebuild")
        .args([
            "-workspace",
            "clients/apple/lockbook.xcworkspace",
            "-scheme",
            "Lockbook",
            "-destination",
            "generic/platform=macOS",
            "-configuration",
            "Release",
            "-archivePath",
            "clients/apple/build/Lockbook-macOS.xcarchive",
            "archive",
        ])
        .current_dir(root())
        .assert_success()?;

    Command::new("./build/Lockbook-macOS.xcarchive/Products/Applications/Lockbook.app/Contents/MacOS/Lockbook")
        .current_dir(apple_dir())
        .assert_success()?;

    Ok(())
}

pub fn apple_device_name_completor(prompt: &str) -> CliResult<Vec<String>> {
    Ok(apple_devices()?
        .into_iter()
        .map(|d| d.device_properties.name)
        .filter(|name| name.starts_with(prompt))
        .collect())
}

/// Resolves a device by exact name, UDID or CoreDevice id, else by a unique
/// case-insensitive name substring where ’ and ' are the same.
pub fn get_device_id(query: &str) -> CliResult<String> {
    let devices = apple_devices()?;
    let fold = |s: &str| s.replace('’', "'").to_lowercase();
    let q = fold(query);

    let exact: Vec<&Device> = devices
        .iter()
        .filter(|d| {
            d.device_properties.name == query
                || fold(&d.identifier) == q
                || d.hardware_properties.udid.as_deref().map(fold) == Some(q.clone())
        })
        .collect();
    let matches = if exact.is_empty() {
        devices
            .iter()
            .filter(|d| fold(&d.device_properties.name).contains(&q))
            .collect()
    } else {
        exact
    };

    let list = |ds: &[&Device]| {
        ds.iter()
            .map(|d| format!("  {}", d.device_properties.name))
            .collect::<Vec<_>>()
            .join("\n")
    };
    match matches.as_slice() {
        [d] => Ok(d.identifier.clone()),
        [] => {
            let all: Vec<&Device> = devices.iter().collect();
            let names = if all.is_empty() { "  (none)".to_string() } else { list(&all) };
            Err(CliError {
                msg: format!(
                    "no connected device matches \"{query}\". Connected devices:\n{names}"
                ),
                status: 1,
            })
        }
        some => Err(CliError {
            msg: format!("\"{query}\" matches more than one device:\n{}", list(some)),
            status: 1,
        }),
    }
}

/// Paired physical devices that aren't unavailable: "available (paired)" or
/// "connected" (tunnel up).
fn apple_devices() -> CliResult<Vec<Device>> {
    let json_path_str = "/tmp/devicectl_devices.json";
    let json_path = PathBuf::from(json_path_str);

    Command::new("xcrun")
        .args([
            "devicectl",
            "list",
            "devices",
            "--hide-default-columns",
            "--filter",
            "Reality == 'physical' AND State != 'unavailable'",
            "-q",
            "-j",
            json_path_str,
        ])
        .assert_success()?;

    let json_str = fs::read_to_string(&json_path)?;
    let root: Root = serde_json::from_str(&json_str)?;
    Ok(root.result.devices)
}

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Root {
    result: ResultField,
}

#[derive(Debug, Deserialize)]
struct ResultField {
    devices: Vec<Device>,
}

#[derive(Debug, Deserialize)]
struct Device {
    identifier: String,

    #[serde(rename = "deviceProperties")]
    device_properties: DeviceProperties,

    #[serde(rename = "hardwareProperties")]
    hardware_properties: HardwareProperties,
}

#[derive(Debug, Deserialize)]
struct DeviceProperties {
    name: String,
}

#[derive(Debug, Deserialize)]
struct HardwareProperties {
    udid: Option<String>,
}

pub fn server() -> CliResult<()> {
    dotenvy::from_path(local_env_path()).unwrap();
    Command::new("cargo")
        .args(["run", "-p", "lockbook-server", "--release"])
        .current_dir(root())
        .assert_success()?;

    Ok(())
}

pub fn apple_run_sim(name: String) -> CliResult<()> {
    let udid = simulator_udid(&name)?;
    apple_ws(WsBuildTargets { ios: false, ios_sim: true, arm_macos: false, x86_macos: false })?;

    let derived_data = apple_dir().join("build/DerivedData-sim");
    Command::new("xcodebuild")
        .args(["-workspace", "clients/apple/lockbook.xcworkspace", "-scheme", "Lockbook"])
        .args(["-destination", &format!("id={udid}"), "-configuration", "Debug"])
        .arg("-derivedDataPath")
        .arg(&derived_data)
        .arg("build")
        .current_dir(root())
        .assert_success()?;

    Command::new("xcrun")
        .args(["simctl", "bootstatus", &udid, "-b"])
        .assert_success()?;
    Command::new("xcrun")
        .args(["simctl", "install", &udid])
        .arg(derived_data.join("Build/Products/Debug-iphonesimulator/Lockbook.app"))
        .assert_success()?;
    Command::new("xcrun")
        .args(["simctl", "launch", "--terminate-running-process", &udid, "app.lockbook"])
        .assert_success()?;
    Ok(())
}

pub fn simulator_name_completor(prompt: &str) -> CliResult<Vec<String>> {
    let mut names: Vec<String> = simulators()?
        .into_iter()
        .map(|sim| sim.name)
        .filter(|name| name.starts_with(prompt))
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

fn simulator_udid(name: &str) -> CliResult<String> {
    let sims = simulators()?;
    let matches: Vec<&Simulator> = sims
        .iter()
        .filter(|sim| sim.udid == name || sim.name == name)
        .collect();
    match matches.as_slice() {
        [sim] => Ok(sim.udid.clone()),
        [] => Err(format!("no available simulator named or with udid '{name}'").into()),
        _ => {
            let rows: Vec<String> = matches
                .iter()
                .map(|sim| format!("  {} {} ({})", sim.udid, sim.name, sim.runtime))
                .collect();
            Err(format!("'{name}' names several simulators, pass a udid:\n{}", rows.join("\n"))
                .into())
        }
    }
}

struct Simulator {
    udid: String,
    name: String,
    runtime: String,
}

fn simulators() -> CliResult<Vec<Simulator>> {
    #[derive(Deserialize)]
    struct SimctlList {
        devices: std::collections::BTreeMap<String, Vec<SimctlDevice>>,
    }
    #[derive(Deserialize)]
    struct SimctlDevice {
        udid: String,
        name: String,
    }

    let out = Command::new("xcrun")
        .args(["simctl", "list", "devices", "available", "-j"])
        .success_output()?;
    let list: SimctlList = serde_json::from_slice(&out.stdout)?;
    Ok(list
        .devices
        .into_iter()
        .flat_map(|(runtime, devices)| {
            let runtime = runtime
                .trim_start_matches("com.apple.CoreSimulator.SimRuntime.")
                .to_string();
            devices.into_iter().map(move |d| Simulator {
                udid: d.udid,
                name: d.name,
                runtime: runtime.clone(),
            })
        })
        .collect())
}

pub fn android_run(device: String) -> CliResult<()> {
    let serial = adb_serial(&device)?;
    let sdk = android_sdk();
    let adb = || {
        let mut cmd = Command::new(adb_path(&sdk));
        cmd.args(["-s", &serial]);
        cmd
    };

    let abi_out = adb()
        .args(["shell", "getprop", "ro.product.cpu.abi"])
        .success_output()?;
    let abi = String::from_utf8_lossy(&abi_out.stdout).trim().to_string();
    let rust_target = match abi.as_str() {
        "arm64-v8a" => "aarch64-linux-android",
        "armeabi-v7a" => "armv7-linux-androideabi",
        "x86_64" => "x86_64-linux-android",
        "x86" => "i686-linux-android",
        _ => return Err(format!("{serial} reports an unknown abi '{abi}'").into()),
    };

    let mut ndk = Command::new("cargo");
    ndk.args(["ndk", "--target", rust_target, "--platform", "22"])
        .args(["--", "build", "--release"])
        .current_dir(workspace_ffi());
    android_env(&mut ndk, &sdk)?.assert_success()?;

    let jni_libs = android_dir().join("workspace/src/main/jniLibs").join(&abi);
    fs::create_dir_all(&jni_libs)?;
    fs::copy(
        target().join(rust_target).join("release/libworkspace.so"),
        jni_libs.join("libworkspace.so"),
    )?;

    let mut gradle = Command::new(android_dir().join("gradlew"));
    gradle
        .arg("assemblePlayDebug")
        .arg(format!("-Pandroid.injected.build.abi={abi}"))
        .current_dir(android_dir());
    android_env(&mut gradle, &sdk)?.assert_success()?;

    adb()
        .args(["install", "-r", "-t", "-d"])
        .arg(android_dir().join("app/build/intermediates/apk/play/debug/app-play-debug.apk"))
        .assert_success()?;
    let launcher = ["-a", "android.intent.action.MAIN", "-c", "android.intent.category.LAUNCHER"];
    let resolved = adb()
        .args(["shell", "cmd", "package", "resolve-activity", "--brief"])
        .args(launcher)
        .arg("app.lockbook")
        .success_output()?;
    let resolved = String::from_utf8_lossy(&resolved.stdout);
    let activity = resolved
        .lines()
        .last()
        .filter(|line| line.contains('/'))
        .ok_or(format!("no launcher activity for app.lockbook on {serial}: {resolved}"))?;
    adb()
        .args(["shell", "am", "start", "-S", "-W", "-n", activity])
        .assert_success()?;
    Ok(())
}

pub fn android_device_completor(prompt: &str) -> CliResult<Vec<String>> {
    Ok(adb_devices()?
        .into_iter()
        .flat_map(|(serial, model)| [Some(serial), model])
        .flatten()
        .filter(|entry| entry.starts_with(prompt))
        .collect())
}

/// Resolves an adb serial or a model name (`Pixel_8_Pro` or `Pixel 8 Pro`) to a serial.
fn adb_serial(device: &str) -> CliResult<String> {
    let devices = adb_devices()?;
    let model = device.replace(' ', "_");
    let matches: Vec<&(String, Option<String>)> = devices
        .iter()
        .filter(|(serial, m)| serial == device || m.as_deref() == Some(model.as_str()))
        .collect();
    match matches.as_slice() {
        [(serial, _)] => Ok(serial.clone()),
        _ => {
            let rows: Vec<String> = devices
                .iter()
                .map(|(serial, model)| format!("  {serial} {}", model.as_deref().unwrap_or("")))
                .collect();
            let problem = if matches.is_empty() { "matches no" } else { "matches several" };
            Err(format!("'{device}' {problem} connected device, adb lists:\n{}", rows.join("\n"))
                .into())
        }
    }
}

/// Serial and model of every device adb can install to.
fn adb_devices() -> CliResult<Vec<(String, Option<String>)>> {
    let out = Command::new(adb_path(&android_sdk()))
        .args(["devices", "-l"])
        .success_output()?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let serial = fields.next()?;
            if fields.next()? != "device" {
                return None;
            }
            let model = fields
                .find_map(|f| f.strip_prefix("model:"))
                .map(String::from);
            Some((serial.to_string(), model))
        })
        .collect())
}

fn android_sdk() -> PathBuf {
    if let Some(sdk) = std::env::var_os("ANDROID_HOME").or(std::env::var_os("ANDROID_SDK_ROOT")) {
        return PathBuf::from(sdk);
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    if cfg!(target_os = "macos") {
        home.join("Library/Android/sdk")
    } else {
        home.join("Android/Sdk")
    }
}

fn adb_path(sdk: &Path) -> PathBuf {
    let adb = sdk.join("platform-tools/adb");
    if adb.exists() { adb } else { PathBuf::from("adb") }
}

/// Gradle and cargo-ndk need the SDK, the NDK version in `ANDROID_NDK_VERSION`, and a JDK;
/// fills in the SDK found above, its newest NDK and Android Studio's JDK when they aren't set.
fn android_env<'a>(cmd: &'a mut Command, sdk: &Path) -> CliResult<&'a mut Command> {
    cmd.env("ANDROID_HOME", sdk);
    if std::env::var_os("ANDROID_NDK_VERSION").is_none() {
        let mut versions: Vec<String> = fs::read_dir(sdk.join("ndk"))
            .map_err(|e| format!("no NDK under {}: {e}", sdk.display()))?
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .collect();
        versions.sort_by_key(|v| {
            v.split('.')
                .map(|n| n.parse::<u64>().unwrap_or(0))
                .collect::<Vec<_>>()
        });
        let newest = versions
            .pop()
            .ok_or(format!("no NDK under {}", sdk.display()))?;
        cmd.env("ANDROID_NDK_VERSION", newest);
    }
    let studio_jdk = Path::new("/Applications/Android Studio.app/Contents/jbr/Contents/Home");
    if std::env::var_os("JAVA_HOME").is_none() && studio_jdk.exists() {
        cmd.env("JAVA_HOME", studio_jdk);
    }
    Ok(cmd)
}
