#!/bin/sh

set -ea

command -V cargo

projRoot=$(git rev-parse --show-toplevel)

cd "$projRoot"/core
. ../containers/local.env

cargo test --release upgrade_account_android_invalid_purchase_token -- --nocapture --ignored
