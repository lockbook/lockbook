#!/bin/sh

set -e

if ! command -v github-release &> /dev/null
then
	echo "You do not have the util github-release, checkout https://github.com/github-release/github-release"
	exit 69
fi

current_branch=$(git rev-parse --abbrev-ref HEAD)
current_hash=$(git rev-parse --short HEAD)

if [ $current_branch != "master" ]
then
	echo "Do not release non-master code."
	exit 69
fi

if [ $(uname) != "Darwin" ]
then
	echo "Not on macOS"
	exit 69
fi

echo "Building release"
cd ../clients/cli
API_URL="http://api.lockbook.app:8000" cargo build --release
cd target/release

