#[cfg(target_os = "linux")]
mod input;
// #[cfg(target_os = "linux")]
// mod output;
#[cfg(target_os = "linux")]
mod window;

#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
fn main() {
    window::main().unwrap()
}
