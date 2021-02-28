cd ..\core
cargo build --target=aarch64-pc-windows-msvc --release --no-default-features --features "native-tls"
copy target\aarch64-pc-windows-msvc\release\lockbook_core.dll ..\clients\windows\core\