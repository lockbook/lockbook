use crate::Secrets;

pub fn release_apple(secret: Secrets) {
    cli::release(secret);
    release_ios();
    release_macos();
}

mod cli {
    use crate::utils::{core_version, lb_repo, sha_file, CommandRunner};
    use crate::Secrets;
    use github_release_rs::ReleaseClient;
    use std::fs;
    use std::fs::File;
    use std::process::Command;

    static CLI_NAME: &str = "lockbook-cli-macos.tar.gz";

    pub fn release(secret: Secrets) {
        build_x86();
        build_arm();
        lipo_binaries();
        tar_binary();
        upload(secret);
        bump_brew();
    }

    fn build_x86() {
        Command::new("cargo")
            .args(["build", "-p", "lockbook-cli", "--release", "--target=x86_64-apple-darwin"])
            .assert_success();
    }

    fn build_arm() {
        Command::new("cargo")
            .args(["build", "-p", "lockbook-cli", "--release", "--target=aarch64-apple-darwin"])
            .assert_success();
    }

    fn lipo_binaries() {
        fs::create_dir_all("target/universal-cli/").unwrap();
        Command::new("lipo")
            .args([
                "-create",
                "-output",
                "target/universal-cli/lockbook",
                "target/x86_64-apple-darwin/release/lockbook",
                "target/aarch64-apple-darwin/release/lockbook",
            ])
            .assert_success();
    }

    fn tar_binary() {
        Command::new("tar")
            .args(["-czf", CLI_NAME, "lockbook"])
            .current_dir("target/universal-cli")
            .assert_success();
    }

    fn tarred_binary() -> String {
        format!("target/universal-cli/{CLI_NAME}")
    }

    fn upload(secret: Secrets) {
        let client = ReleaseClient::new(secret.gh_token).unwrap();
        let release = client
            .get_release_by_tag_name(&lb_repo(), &core_version())
            .unwrap();
        let file = File::open(tarred_binary()).unwrap();
        client
            .upload_release_asset(
                &lb_repo(),
                release.id as u64,
                "lockbook-cli-macos.tar.gz",
                "application/gzip",
                file,
                None,
            )
            .unwrap();
    }

    fn bump_brew() {}

    fn overwrite_lockbook_rb() {
        let version = core_version();
        let sha = sha_file(&tarred_binary());

        let new_content = format!(
            r#"
class Lockbook < Formula
  desc "The best place to store and share thoughts."
  homepage "https://github.com/lockbook/lockbook"
  url "https://github.com/lockbook/lockbook/releases/download/{version}/{CLI_NAME}"
  sha256 "{sha}"
  version "{version}"

  def install
    bin.install "lockbook"
  end
end
"#
        );
    }
}

fn release_ios() {
    todo!()
}

fn release_macos() {
    todo!()
}
