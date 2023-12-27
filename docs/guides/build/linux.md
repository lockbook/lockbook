# Build Linux

Prerequisites:
- Computer with a linux distro
- Stable rust toolchain

Most linux distros require the installation of the following packages:
+ `build-essential`
+ `libssl-devel`
+ `pkg-config`
+ `libgtk-3-dev` (for rfd)
+ `libgtk-4-dev`
+ `libadwaita-1-dev`

Steps:
- In `/clients/egui` run `cargo build`
