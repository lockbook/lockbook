use std::io::Write;
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

#[test]
fn closed_stdout_still_syncs() {
    let api_url = env::var("API_URL").unwrap_or("http://localhost:8000".into());
    let data = env::temp_dir().join(format!("lockbook-cli-sync-pipe-{}", process::id()));
    let lockbook = |args: &[&str]| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lockbook"));
        cmd.args(args).env("LOCKBOOK_PATH", &data);
        cmd
    };
    fs::remove_dir_all(&data).ok();
    let username = format!(
        "syncpipe{}{}",
        process::id(),
        std::time::UNIX_EPOCH.elapsed().unwrap().as_millis()
    );
    let created = lockbook(&["account", "new", &username, &format!("--api_url={api_url}")])
        .output()
        .unwrap();
    assert!(created.status.success(), "{}", String::from_utf8_lossy(&created.stderr));
    for i in 0..30 {
        let mut write = lockbook(&["write", &format!("/note{i}.md")])
            .stdin(process::Stdio::piped())
            .spawn()
            .unwrap();
        write.stdin.take().unwrap().write_all(b"hi").unwrap();
        assert!(write.wait().unwrap().success());
    }

    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let synced = lockbook(&["sync"]).stdout(writer).output().unwrap();
    let resync = lockbook(&["sync"]).output().unwrap();
    fs::remove_dir_all(&data).ok();

    assert!(synced.status.success(), "{}", String::from_utf8_lossy(&synced.stderr));
    assert!(resync.status.success(), "{}", String::from_utf8_lossy(&resync.stderr));
    assert!(!String::from_utf8_lossy(&resync.stdout).contains("Uploading"));
}
