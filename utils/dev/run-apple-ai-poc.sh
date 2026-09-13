#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."

# Reuse the installed Mac app's account when there is no desktop account yet.
# An explicit LOCKBOOK_PATH always wins.
if [[ -z "${LOCKBOOK_PATH:-}" && ! -d "$HOME/.lockbook" && -d "$HOME/Library/Containers/app.lockbook/Data/.lockbook" ]]; then
    export LOCKBOOK_PATH="$HOME/Library/Containers/app.lockbook/Data/.lockbook"
fi
if pgrep -x Lockbook >/dev/null || pgrep -f '^.*[/]lockbook-desktop$' >/dev/null; then
    echo 'Quit the running Lockbook app before launching the preview (they share an account database).'
    exit 1
fi
cargo build -p lockbook-desktop
exec ./target/debug/lockbook-desktop
