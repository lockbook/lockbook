# Windows Building

Currently, only the x64 architecture is supported. Other versions should build but you may experience issues or need to modify a build script.

## Setting Up Build Environment

- Install Rust
    - install the toolchain for your architecture
    - if you intend to build releases, install toolchains for all architectures with `rustup target add i686-pc-windows-msvc x86_64-pc-windows-msvc aarch64-pc-windows-msvc`
- Install Visual Studio 2022 (we have experienced issues with other versions)
    - include the `Universal Windows Platform Development` workload
    - include the latest `VS 2022 C++ Build Tools` for your platform (all platforms if you intend to build releases)

## Building

- In `lockbook/utils/dev`, run `create_core_for_windows.bat`, which builds the lockbook core dll and places it where the C# project expects it
- Open `lockbook.sln` in Visual Studio, select your architecture, and run
