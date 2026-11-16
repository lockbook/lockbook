#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        eprintln!(
            "lockbook-desktop — Lockbook desktop app\n\
             \n\
             Usage:\n\
               lockbook-desktop [<link>]\n\
               lockbook-desktop --help\n\
             \n\
             Links:\n\
               lb://<id>#<heading>                              a note, at a heading\n\
               lb://search?q=<query>&type=content&scope=<path>  a search"
        );
        return;
    }

    lockbook_desktop::run();
}
