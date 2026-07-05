use std::{fs, io};

use crate::util::data_dir;

/// Persisted user preferences. Trimmed to the bare minimum for the 2026 rewrite;
/// fields will grow back as screens land. `#[serde(default)]` means an existing
/// on-disk settings.json with extra keys still deserializes cleanly.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// winit's Wayland backend drops file drag-and-drop, so the desktop shell
    /// defaults to X11 on Linux unless the user opts in here.
    #[cfg(target_os = "linux")]
    pub allow_wayland: bool,
}

impl Settings {
    pub fn read_from_file() -> Result<Self, Box<dyn std::error::Error>> {
        let path = format!("{}/egui/settings.json", data_dir()?);
        match fs::File::open(&path) {
            Ok(f) => Ok(serde_json::from_reader(f)?),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(Box::new(err)),
        }
    }
}
