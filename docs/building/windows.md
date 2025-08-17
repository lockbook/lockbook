# Build Windows

Prerequisites:

- Computer with Windows
- Stable rust toolchain

Steps:

### egui vs windows

The Lockbook windows release is built with `/clients/windows`. Sometimes it can be helpful to know if a bug is reproducible in egui and/or the windows client.

- In `/clients/windows` run `cargo build`
- In `/clients/egui` run `cargo build`
