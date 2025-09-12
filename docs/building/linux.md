# Build Linux

Prerequisites:
- Computer with a linux distro
- Stable rust toolchain

Most linux distros require the installation of the following packages:
+ `build-essential`
+ `libssl-devel`
+ `pkg-config`
+ `libxkbcommon-x11-dev`
+ `libgtk-3-dev` (for rfd)

Steps:
- In `/clients/linux` run `cargo build`

Nix users can start a `nix-shell` in the `clients/linux` directory which will configure all the dependencies.