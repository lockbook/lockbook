use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // git appends to the HEAD reflog on every commit, checkout and reset, packed refs or not.
    if let Some(reflog) = git(&["rev-parse", "--git-path", "logs/HEAD"]) {
        if std::path::Path::new(&reflog).exists() {
            println!("cargo:rerun-if-changed={reflog}");
        }
    }
    let commit = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=LB_GIT_COMMIT={commit}");
}
