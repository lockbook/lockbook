#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
fn main() {}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn main() {
    lockbook_desktop::run();
}
