#!/bin/sh
set -eu

# Always run so Xcode-only builds record their own checkout, not the Rust library's.
cd "$SRCROOT/../.."
commit=$(git rev-parse HEAD)
status=0
git diff --quiet HEAD -- || status=$?
case "$status" in
    0) ;;
    1) commit="$commit-dirty" ;;
    *) exit "$status" ;;
esac

source="enum BuildInfo {
    static let identity = \"bundle=$PRODUCT_BUNDLE_IDENTIFIER version=$MARKETING_VERSION build=$CURRENT_PROJECT_VERSION commit=$commit configuration=$CONFIGURATION\"
}"
if [ "$(cat "$SCRIPT_OUTPUT_FILE_0" 2>/dev/null || true)" != "$source" ]; then
    printf '%s\n' "$source" > "$SCRIPT_OUTPUT_FILE_0"
fi
