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
    println!("cargo:rerun-if-changed=src");
    // HEAD changes when switching branches; its symbolic ref changes when committing.
    for reference in [Some("HEAD".to_owned()), git(&["symbolic-ref", "-q", "HEAD"])]
        .into_iter()
        .flatten()
    {
        if let Some(path) = git(&["rev-parse", "--git-path", &reference]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    let mut commit = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    if Command::new("git")
        .args(["diff", "--quiet", "HEAD", "--"])
        .status()
        .ok()
        .and_then(|status| status.code())
        == Some(1)
    {
        commit.push_str("-dirty");
    }
    println!("cargo:rustc-env=LB_GIT_COMMIT={commit}");
}
