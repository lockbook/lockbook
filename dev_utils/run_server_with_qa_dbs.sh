#!/bin/sh

set -a
. ../containers/qa.env
cd ../server
RUST_LOG=debug cargo run
