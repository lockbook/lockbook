use std::{fs, io, path::PathBuf};

use crate::components::{ModePreference, ThemeFamily};
use crate::util::data_dir;
use workspace_rs::theme::palette_v2::Palette;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_mode: ModePreference,
    #[serde(rename = "theme_name")]
    pub theme_family: ThemeFamily,
    /// Overrides the theme's accent; cleared when the theme changes.
    pub theme_accent: Option<Palette>,
    pub sidebar_usage: bool,
    pub zen_mode: bool, // hide side panel and maximize the content workspace
    /// installed family names; unset draws in the platform's
    pub text_font: Option<String>,
    pub code_font: Option<String>,
    #[cfg(target_os = "linux")]
    pub allow_wayland: bool,
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
        let path = format!("{}/egui/settings.json", data_dir()?);
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

    /// Load from disk, or defaults with a writable path when the file is missing
    /// / unreadable (shell always keeps a live [`Settings`]).
    pub fn load() -> Self {
        match Self::read_from_file() {
            Ok(s) => s,
            Err(_) => {
                let mut s = Self::default();
                if let Ok(dir) = data_dir() {
                    let egui_dir = format!("{dir}/egui");
                    let _ = fs::create_dir_all(&egui_dir);
                    s.path = format!("{egui_dir}/settings.json");
                }
                s
            }
        }
    }

    pub fn to_file(&self) -> io::Result<()> {
        if self.path.is_empty() {
            return Ok(());
        }
        if let Some(parent) = PathBuf::from(&self.path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        let content = serde_json::to_string(self).ok().unwrap();
        fs::write(&self.path, content)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_mode: ModePreference::System,
            theme_family: ThemeFamily::default(),
            theme_accent: None,
            sidebar_usage: true,
            path: "".to_string(),
            zen_mode: false,
            text_font: None,
            code_font: None,
            #[cfg(target_os = "linux")]
            allow_wayland: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_family_round_trips_and_unknown_names_save_as_the_default() {
        for family in ThemeFamily::ALL {
            let saved = Settings { theme_family: family, ..Default::default() };
            let json = serde_json::to_string(&saved).unwrap();
            let loaded: Settings = serde_json::from_str(&json).unwrap();
            assert_eq!(loaded.theme_family, family, "{json}");
        }

        let old: Settings = serde_json::from_str(r#"{"theme_name":"default"}"#).unwrap();
        assert_eq!(old.theme_family, ThemeFamily::default());
        let resaved = serde_json::to_string(&old).unwrap();
        assert!(resaved.contains(r#""theme_name":"mnemonic""#), "{resaved}");
    }
}
