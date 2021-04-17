use std::error::Error;
use std::fs;
use std::fs::File;
use std::io;

use serde::{Deserialize, Serialize};

#[serde(default)]
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub hidden_tree_cols: Vec<String>,
    pub window_maximize: bool,
    pub auto_save: bool,
    #[serde(skip_serializing, skip_deserializing)]
    path: String,
}

impl Settings {
    pub fn from_data_dir(dir: &str) -> Result<Self, Box<dyn Error>> {
        let path = format!("{}/settings.yaml", dir);
        let mut s: Self = match File::open(&path) {
            Ok(f) => serde_yaml::from_reader(f)?,
            Err(err) => match err.kind() {
                io::ErrorKind::NotFound => Self::default(),
                _ => return Err(Box::new(err)),
            },
        };
        s.path = path;
        Ok(s)
    }

    pub fn to_file(&self) -> io::Result<()> {
        let content = serde_yaml::to_string(&self).ok().unwrap();
        fs::write(&self.path, &content)
    }

    pub fn toggle_tree_col(&mut self, col: String) {
        let cols = &mut self.hidden_tree_cols;
        if cols.contains(&col) {
            cols.retain(|c| !c.eq(&col));
        } else {
            cols.push(col);
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hidden_tree_cols: vec!["Id".to_string(), "Type".to_string()],
            window_maximize: false,
            auto_save: true,
            path: "".to_string(),
        }
    }
}
