use std::fs;
use std::io;

use crate::util::data_dir;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_mode: ThemeMode,
    pub theme_color: lb::ColorAlias,
    pub window_maximize: bool,
    pub open_new_files: bool,
    pub auto_save: bool,
    pub auto_sync: bool,
    pub sidebar_usage: bool,
    #[serde(skip_serializing, skip_deserializing)]
    path: String,
}

impl Settings {
    pub fn read_from_file() -> Result<Self, Box<dyn std::error::Error>> {
        let path = match data_dir() {
            Ok(dir) => format!("{}/egui/settings.json", dir),
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
        fs::write(&self.path, &content)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            theme_color: lb::ColorAlias::Blue,
            window_maximize: false,
            open_new_files: true,
            auto_save: true,
            auto_sync: true,
            sidebar_usage: true,
            path: "".to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThemeMode {
    System,
    Dark,
    Light,
}
