cd ..\core
cargo build --target=aarch64-pc-windows-msvc --release
copy target\aarch64-pc-windows-msvc\release\lockbook_core.dll ..\clients\windows\core\