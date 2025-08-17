# Build Windows

Prerequisites:

- Computer with Windows
- Stable rust toolchain

Steps:

### egui vs windows

Lockbook windows release is build with `/clients/windows` but sometimes it can be helpful to know if a bug is reproducible in the egui client on windows vs the windows client.

- In `/clients/windows` run `cargo build`
- In `/clients/egui` run `cargo build`
