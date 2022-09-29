#!/bin/sh

set -ea

command -V cargo

projRoot=$(git rev-parse --show-toplevel)

cd "$projRoot"/core
. ../containers/local.env

cargo test --test exhaustive_sync_check --release -- --nocapture --ignored
