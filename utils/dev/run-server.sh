#!/bin/sh

set -ea

projRoot=`git rev-parse --show-toplevel`
cd $projRoot/server/server

dir="$1"
if [ -z "$dir" ]; then
	dir="/tmp/lbdev"
fi

printf "Starting redis server... "
redis-server > redis-server.log 2>&1 &
printf "Done. PID: $! \n"

printf "Starting minio server... "
minio server $dir > minio-server.log 2>&1 &

. ../../containers/local.env

while ! nc -z $FILES_DB_HOST $FILES_DB_PORT
do
	sleep 0.2
done
printf "Done. PID: $! \n"

echo "Configuring minio..."
mc config host add filesdb $FILES_DB_SCHEME://$FILES_DB_HOST:$FILES_DB_PORT $FILES_DB_ACCESS_KEY $FILES_DB_SECRET_KEY
mc mb -p --region=$FILES_DB_REGION filesdb/$FILES_DB_BUCKET
mc policy set public filesdb/$FILES_DB_BUCKET

echo "Compiling and running lockbook server..."
cargo run
