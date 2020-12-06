#!/bin/sh

set -a
. ../containers/test.env
cd ../server
RUST_LOG=info cargo run
