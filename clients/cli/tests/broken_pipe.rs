use std::process::Command;
use std::{env, fs, process};

#[test]
fn closed_stdout_ends_quietly() {
    let data = env::temp_dir().join(format!("lockbook-cli-broken-pipe-{}", process::id()));
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);

    let output = Command::new(env!("CARGO_BIN_EXE_lockbook"))
        .args(["account", "new", "broken-pipe", "--api_url=http://127.0.0.1:9"])
        .env("LOCKBOOK_PATH", &data)
        .stdout(writer)
        .output()
        .unwrap();
    let panics = fs::read_dir(&data)
        .map(|dir| {
            dir.flatten()
                .filter(|f| f.file_name().to_string_lossy().starts_with("panic"))
                .count()
        })
        .unwrap_or(0);
    fs::remove_dir_all(&data).ok();

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(panics, 0);
}
