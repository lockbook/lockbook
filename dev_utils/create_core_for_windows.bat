set "API_URL=http://api.lockbook.app:8000"
cd ..\core
cargo clean
cargo build --release
copy target\release\lockbook_core.dll ..\clients\windows\