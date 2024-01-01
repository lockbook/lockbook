# Build Linux

Prerequisites:
- Computer with a linux distro
- Stable rust toolchain

Most linux distros require the installation of the following packages:
+ `build-essential`
+ `libssl-devel`
+ `pkg-config`
+ `libgtk-3-dev` (for rfd)

Steps:
- In `/clients/egui` run `cargo build`
