//! `chrome = "macos" | "windows" | "linux"`: which window-chrome variant the
//! shell paints. Follows the target OS unless `win-chrome` is on, which draws
//! the Windows captions anywhere (store screenshots captured on a Mac).
fn main() {
    println!("cargo::rustc-check-cfg=cfg(chrome, values(\"macos\", \"windows\", \"linux\"))");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let chrome = match (os.as_str(), std::env::var_os("CARGO_FEATURE_WIN_CHROME").is_some()) {
        (_, true) | ("windows", _) => "windows",
        ("macos", _) => "macos",
        _ => "linux",
    };
    println!("cargo::rustc-cfg=chrome=\"{chrome}\"");
}
