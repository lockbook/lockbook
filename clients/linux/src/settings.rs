use std::cell::RefCell;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub hidden_tree_cols: Vec<String>,

    #[serde(default)]
    pub window_maximize: bool,

    #[serde(skip_serializing, skip_deserializing)]
    path: String,
}

impl Settings {
    pub fn default() -> Self {
        Self {
            hidden_tree_cols: vec![],
            window_maximize: false,
            path: "".to_string(),
        }
    }

    pub fn new_rc(s: Settings) -> Rc<RefCell<Settings>> {
        Rc::new(RefCell::new(s))
    }

    pub fn from_file(path: &str) -> Result<Self, Box<dyn Error>> {
        let mut s: Settings = match File::open(path) {
            Ok(f) => serde_yaml::from_reader(f)?,
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => Settings::default(),
                _ => return Err(Box::new(err)),
            },
        };
        s.path = path.to_string();
        Ok(s)
    }

    pub fn to_file(&self) -> std::io::Result<()> {
        let content = serde_yaml::to_string(self).ok().unwrap();
        fs::write(&self.path, &content)?;
        Ok(())
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
