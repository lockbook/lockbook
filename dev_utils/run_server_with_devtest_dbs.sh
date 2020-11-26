#!/bin/sh

set -a
. ../containers/dev-test.env
cd ../server
RUST_LOG=info cargo run
