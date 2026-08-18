#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        eprintln!(
            "lockbook-desktop — Lockbook desktop app\n\
             \n\
             Usage:\n\
               lockbook-desktop\n\
               lockbook-desktop --help"
        );
        return;
    }

    lockbook_desktop::run();
}
