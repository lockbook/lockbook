use lazy_static::lazy_static;
use prometheus::{IntGaugeVec, register_int_gauge_vec};

lazy_static! {
    pub static ref INSTALLS: IntGaugeVec = register_int_gauge_vec!(
        "lb_installs",
        "Install/download counts by distribution channel, client, and OS",
        &["distribution_channel", "client", "os", "country"]
    )
    .unwrap();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Normalized {
    pub client: &'static str, // cli, linux, windows, macos, ios, android
    pub os: &'static str,     // linux, windows, apple, android
}

/// Normalizes product names to client type and OS.
/// Clients: cli, linux, windows, macos, ios, android
/// OS: linux, windows, apple, android
pub fn normalize_github_asset(name: &str) -> Option<Normalized> {
    let name = name.to_lowercase();

    // Filter out server builds
    if name.contains("server") {
        return None;
    }

    // Android
    if name.contains("android") || name == "app-release.apk" {
        return Some(Normalized { client: "android", os: "android" });
    }

    // CLI - need to determine OS
    if name.contains("cli") || name.ends_with(".deb") {
        let os = if name.contains("windows") {
            "windows"
        } else if name.contains("macos") || name.contains("apple") {
            "apple"
        } else {
            "linux" // default for CLI, .deb, etc.
        };
        return Some(Normalized { client: "cli", os });
    }

    // Desktop by OS
    if name.contains("windows") || name.ends_with(".exe") || name.ends_with(".msixbundle") {
        return Some(Normalized { client: "windows", os: "windows" });
    }
    if name.contains("macos") {
        return Some(Normalized { client: "macos", os: "apple" });
    }
    if name.contains("linux") || name.contains("egui") {
        return Some(Normalized { client: "linux", os: "linux" });
    }

    None
}

pub fn normalize_app_store(product: &str, product_type: &str) -> Option<Normalized> {
    // Filter out non-app products
    if product.to_lowercase().contains("premium") {
        return None;
    }

    // Product Type Identifier:
    // 1, 1F, 1T, F1, etc. = iOS/iPadOS
    // 7, 7F, 7T, F7, etc. = macOS
    let first_digit = product_type.chars().find(|c| c.is_ascii_digit());
    match first_digit {
        Some('7') => Some(Normalized { client: "macos", os: "apple" }),
        Some('1') => Some(Normalized { client: "ios", os: "apple" }),
        _ => Some(Normalized { client: "ios", os: "apple" }), // Default to iOS
    }
}
