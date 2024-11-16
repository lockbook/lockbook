#!/bin/sh

set -ea

projRoot=`git rev-parse --show-toplevel`

cd $projRoot
. containers/local.env

echo "Compiling and running lockbook server..."
cd server
RUST_MIN_STACK=104857600 cargo run $@
