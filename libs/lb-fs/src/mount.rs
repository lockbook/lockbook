use std::fs;
use tokio::process::Command;
use tracing::info;

// see https://github.com/xetdata/nfsserve for more mount examples
#[cfg(target_os = "macos")]
pub fn mount() {
    fs::create_dir_all("/tmp/lockbook").unwrap();

    Command::new("mount_nfs")
        .arg("-o")
        .arg("nolocks,vers=3,tcp,rsize=131072,actimeo=120,port=11111,mountport=11111")
        .arg("localhost:/")
        .arg("/tmp/lockbook")
        .spawn()
        .unwrap();
}

// see https://github.com/xetdata/nfsserve for more mount examples
#[cfg(target_os = "linux")]
pub fn mount() {
    fs::create_dir_all("/tmp/lockbook").unwrap();

    Command::new("sudo")
        .arg("mount.nfs")
        .arg("-o")
        .arg("user,noacl,nolock,vers=3,tcp,wsize=1048576,rsize=131072,actimeo=120,port=11111,mountport=11111")
        .arg("localhost:/")
        .arg("/tmp/lockbook")
        .spawn()
        .unwrap();
}

#[cfg(target_os = "windows")]
pub fn mount() {
    fs::create_dir_all("/tmp/lockbook").unwrap();

    Command::new("mount.exe")
        .arg("-o")
        .arg("anon,nolock,mtype=soft,fileaccess=6,casesensitive,lang=ansi,rsize=128,wsize=128,timeout=60,retry=2")
        .arg("localhost:/")
        .arg("/tmp/lockbook")
        .spawn()
        .unwrap();
}

#[cfg(target_os = "linux")]
pub async fn umount() -> bool {
    info!("umounting");
    let wait_result = Command::new("sudo")
        .arg("umount")
        .arg("/tmp/lockbook")
        .spawn()
        .unwrap()
        .wait()
        .await
        .unwrap();

    wait_result.success()
}

#[cfg(target_os = "macos")]
pub async fn umount() -> bool {
    info!("umounting");
    let wait_result = Command::new("umount")
        .arg("/tmp/lockbook")
        .spawn()
        .unwrap()
        .wait()
        .await
        .unwrap();

    wait_result.success()
}

#[cfg(target_os = "windows")]
pub async fn umount() -> bool {
    info!("todo");
    todo!()
}
