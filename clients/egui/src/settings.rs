use std::{fs, io, path::PathBuf};

use crate::util::data_dir;
use workspace_rs::theme::palette_v2::{Mode, Theme};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_mode: ThemeMode,
    pub theme_name: String, // "default" uses built-in, otherwise loads from themes/<name>.json
    pub window_maximize: bool,
    pub open_new_files: bool,
    pub sidebar_usage: bool,
    pub zen_mode: bool, // hide side panel and maximize the content workspace
    #[serde(skip_serializing, skip_deserializing)]
    path: String,
}

impl Settings {
    pub fn write_zen_mode(&mut self, new_value: bool) -> io::Result<()> {
        if self.zen_mode == new_value {
            return Ok(());
        }

        self.zen_mode = new_value;
        self.to_file()
    }

    pub fn read_from_file() -> Result<Self, Box<dyn std::error::Error>> {
        let path = match data_dir() {
            Ok(dir) => format!("{dir}/egui/settings.json"),
            Err(err) => return Err(err.into()),
        };
        let mut s: Self = match fs::File::open(&path) {
            Ok(f) => serde_json::from_reader(f)?,
            Err(err) => match err.kind() {
                io::ErrorKind::NotFound => Self::default(),
                _ => return Err(Box::new(err)),
            },
        };
        s.path = path;
        Ok(s)
    }

    pub fn to_file(&self) -> io::Result<()> {
        let content = serde_json::to_string(&self).ok().unwrap();
        fs::write(&self.path, content)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            theme_name: "default".to_string(),
            window_maximize: false,
            open_new_files: true,
            sidebar_usage: true,
            path: "".to_string(),
            zen_mode: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThemeMode {
    System,
    Dark,
    Light,
}

pub fn themes_dir() -> Option<PathBuf> {
    data_dir()
        .ok()
        .map(|d| PathBuf::from(d).join("egui").join("themes"))
}

pub fn ensure_themes_dir() {
    let Some(dir) = themes_dir() else { return };

    if dir.exists() {
        return;
    }

    if fs::create_dir_all(&dir).is_err() {
        return;
    }

    // Write darcula as a pre-defined alternate theme
    let darcula = Theme::darcula(Mode::Light);
    if let Ok(json) = serde_json::to_string_pretty(&darcula) {
        let _ = fs::write(dir.join("darcula.json"), json);
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
