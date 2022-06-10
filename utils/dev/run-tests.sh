#!/bin/sh

set -ea

command -V cargo

projRoot=$(git rev-parse --show-toplevel)

cd "$projRoot"/core
. ../containers/local.env

cargo test --no-fail-fast -- --nocapture $1
