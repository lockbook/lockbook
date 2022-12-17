use std::fs;
use std::io;

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hidden_tree_cols: Vec<String>,
    pub window_maximize: bool,
    pub open_new_files: bool,
    pub auto_save: bool,
    pub auto_sync: bool,
    #[serde(skip_serializing, skip_deserializing)]
    path: String,
}

impl Settings {
    pub fn from_data_dir(dir: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let path = format!("{}/settings.yaml", dir);
        let mut s: Self = match fs::File::open(&path) {
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
        fs::write(&self.path, content)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hidden_tree_cols: vec!["Id".to_string(), "Type".to_string()],
            window_maximize: false,
            open_new_files: true,
            auto_save: true,
            auto_sync: true,
            path: "".to_string(),
        }
    }
}
