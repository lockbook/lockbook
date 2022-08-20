#[cfg(windows)]
fn main() {
    use std::env;
    use std::fs;
    use std::io;

    use mslnk::ShellLink;

    println!("Installing Lockbook...");

    let appdata = env::var("appdata").unwrap();
    let local_appdata = env::var("localappdata").unwrap();

    let install_dir = format!("{}\\Lockbook", local_appdata);
    fs::create_dir(&install_dir).unwrap_or_else(|err| match err.kind() {
        io::ErrorKind::AlreadyExists => {}
        _ => panic!("{}", err),
    });

    let exe_file = format!("{}\\Lockbook.exe", install_dir);
    let exe_bytes = include_bytes!("../../../target/release/lockbook-egui.exe");
    fs::write(&exe_file, exe_bytes).unwrap();

    let lnk_dir = format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", appdata);
    fs::create_dir(&lnk_dir).unwrap_or_else(|err| match err.kind() {
        io::ErrorKind::AlreadyExists => {}
        _ => panic!("{}", err),
    });

    let sl = ShellLink::new(exe_file).unwrap();
    sl.create_lnk(&format!("{}\\Lockbook.lnk", lnk_dir))
        .unwrap();

    println!("Done.");
}

#[cfg(not(windows))]
fn main() {}
