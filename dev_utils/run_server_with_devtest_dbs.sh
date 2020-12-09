#!/bin/sh

set -a
. ../containers/test.env
cd ../server
INDEX_DB_HOST=localhost
FILES_DB_HOST=localhost
RUST_LOG=info 
cargo run
