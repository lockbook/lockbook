set "API_URL=http://qa.lockbook.app:8000"
cd ..\core
cargo build --release
copy target\release\lockbook_core.dll ..\clients\windows\