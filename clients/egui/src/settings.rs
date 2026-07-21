use std::{fs, io, path::PathBuf};

use workspace_rs::theme::palette_v2::{Mode, Theme};

use crate::util::data_dir;

/// Persisted user preferences.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Light / dark / follow OS.
    pub theme_mode: ThemeMode,
    /// `"default"` uses the built-in palette; otherwise `themes/<name>.json`.
    pub theme_name: String,

    /// winit's Wayland backend drops file drag-and-drop, so the desktop shell
    /// defaults to X11 on Linux unless the user opts in here.
    #[cfg(target_os = "linux")]
    pub allow_wayland: bool,

    /// Fetch markdown link previews (contacts linked sites — privacy tradeoff).
    pub contact_linked_sites: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            theme_name: "default".into(),
            #[cfg(target_os = "linux")]
            allow_wayland: false,
            contact_linked_sites: false,
        }
    }
}

impl Settings {
    pub fn read_from_file() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::path()?;
        match fs::File::open(&path) {
            Ok(f) => Ok(serde_json::from_reader(f)?),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(Box::new(err)),
        }
    }

    pub fn write_to_file(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::path()?;
        if let Some(parent) = std::path::Path::new(&path).parent() {
            fs::create_dir_all(parent)?;
        }
        let f = fs::File::create(&path)?;
        serde_json::to_writer_pretty(f, self)?;
        Ok(())
    }

    fn path() -> Result<String, Box<dyn std::error::Error>> {
        Ok(format!("{}/egui/settings.json", data_dir()?))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeMode {
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

pub fn themes_dir() -> Option<PathBuf> {
    data_dir()
        .ok()
        .map(|d| PathBuf::from(d).join("egui").join("themes"))
}

/// Seed the themes directory with built-in alternates (once).
pub fn ensure_themes_dir() {
    let Some(dir) = themes_dir() else {
        return;
    };
    if dir.exists() {
        return;
    }
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    for (name, theme) in [
        ("darcula", Theme::darcula(Mode::Light)),
        ("catppuccin", Theme::catppuccin(Mode::Light)),
        ("intellij", Theme::intellij(Mode::Light)),
        ("vscode", Theme::vscode(Mode::Light)),
    ] {
        if let Ok(json) = serde_json::to_string_pretty(&theme) {
            let _ = fs::write(dir.join(format!("{name}.json")), json);
        }
    }
}

pub fn list_themes() -> Vec<String> {
    let mut themes = vec!["default".to_string()];
    if let Some(dir) = themes_dir() {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if stem != "default" {
                            themes.push(stem.to_string());
                        }
                    }
                }
            }
        }
    }
    themes.sort();
    themes
}

pub fn load_theme(name: &str, mode: Mode) -> Option<Theme> {
    if name == "default" {
        return None;
    }
    let path = themes_dir()?.join(format!("{name}.json"));
    let file = fs::File::open(path).ok()?;
    let theme: Theme = serde_json::from_reader(file).ok()?;
    Some(theme.with_mode(mode))
}
