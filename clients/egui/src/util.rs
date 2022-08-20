use std::env;

use eframe::egui;

pub fn data_dir() -> Result<String, String> {
    match (env::var("LOCKBOOK_PATH"), env::var("HOME"), env::var("HOMEPATH")) {
        (Ok(s), _, _) => Ok(s),
        (Err(_), Ok(s), _) => Ok(format!("{s}/.lockbook")),
        (Err(_), Err(_), Ok(s)) => Ok(format!("{s}/.lockbook")),
        _ => Err("Unable to determine a Lockbook data directory. Please consider setting the LOCKBOOK_PATH environment variable.".to_string()),
    }
}

pub const NUM_KEYS: [egui::Key; 9] = [
    egui::Key::Num1,
    egui::Key::Num2,
    egui::Key::Num3,
    egui::Key::Num4,
    egui::Key::Num5,
    egui::Key::Num6,
    egui::Key::Num7,
    egui::Key::Num8,
    egui::Key::Num9,
];
