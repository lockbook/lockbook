#!/bin/sh

set -ea

command -V redis-server
command -V minio
command -V mc

projRoot=`git rev-parse --show-toplevel`

cd $projRoot/server/server
cargo build $@

cd $projRoot
if [ -z "$DATA_DIR" ]; then
	DATA_DIR="/tmp/lbdev"
fi
mkdir -p $DATA_DIR
cd $DATA_DIR

printf "Starting redis server... "
redis-server > redis-server.log 2>&1 &
printf "Done. PID: $! \n"

printf "Starting minio server... "
minio server $DATA_DIR > minio-server.log 2>&1 &
minioPID="$!"
sleep 1

cd $projRoot
. containers/local.env

while true; do
	minioListenPID="$(lsof -Pi :$FILES_DB_PORT -sTCP:LISTEN -t || echo)"
	if [ "$minioPID" = "$minioListenPID" ] ; then
		break
	fi
	sleep 0.2
done
printf "Done. PID: $minioPID \n"

echo "Configuring minio..."
mc config host add filesdb $FILES_DB_SCHEME://$FILES_DB_HOST:$FILES_DB_PORT $FILES_DB_ACCESS_KEY $FILES_DB_SECRET_KEY
mc mb -p --region=$FILES_DB_REGION filesdb/$FILES_DB_BUCKET
mc policy set public filesdb/$FILES_DB_BUCKET

echo "Compiling and running lockbook server..."
cd server/server
cargo run $@
