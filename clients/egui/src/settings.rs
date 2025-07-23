use std::{fs, io};

use workspace_rs::theme::palette::ColorAlias;

use crate::util::data_dir;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_mode: ThemeMode,
    pub theme_color: ColorAlias,
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
            theme_color: ColorAlias::Blue,
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
