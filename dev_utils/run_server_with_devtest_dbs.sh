#!/bin/sh

set -a
. ../containers/devtest.env
cd ../server
RUST_LOG=info cargo run
