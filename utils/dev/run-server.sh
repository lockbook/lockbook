#!/bin/sh

set -ea

command -V redis-server
command -V minio
command -V mc

projRoot=`git rev-parse --show-toplevel`

cd $projRoot
if [ -z "$DATA_DIR" ]; then
	DATA_DIR="/tmp/lbdev"
fi
mkdir -p $DATA_DIR
cd $DATA_DIR

cd $projRoot
. containers/local.env

echo "Compiling and running lockbook server..."
cd server/server
cargo run $@
