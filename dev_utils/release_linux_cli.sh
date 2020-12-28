#!/bin/sh

set -ae

if [ -z "$GITHUB_TOKEN" ]
then
	echo "No GITHUB_TOKEN you won't be able to upload to github without this"
	exit 69
fi

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

echo "Performing clean build"
cd ../clients/cli
current_version=$(grep '^version =' Cargo.toml|head -n1|cut -d\" -f2|cut -d\- -f1)
cargo clean
API_URL="http://api.lockbook.app:8000" cargo build --release
cd target/release

echo "taring"
tar -czf lockbook-cli-linux.tar.gz lockbook
sha_description=$(shasum -a 256 lockbook-cli-linux.tar.gz)
sha=$(echo $sha_description | cut -d ' ' -f 1)

echo "Releasing..."

github-release release \
	--user lockbook \
	--repo lockbook \
	--tag "cli-$current_version" \
	--name "Lockbook CLI" \
	--description "0 Dependency Binary. Simply un-tar and place upon path. Repeat to upgrade versions" \
	--pre-release || echo "Failed to create release, perhaps because one exists, attempting upload"

github-release upload \
	--user lockbook \
	--repo lockbook \
	--tag "cli-$current_version" \
	--name "lockbook-cli-linux.tar.gz" \
	--file lockbook-cli-linux.tar.gz

echo $sha_description >> LINUX_CLI_SHA256

github-release upload \
	--user lockbook \
	--repo lockbook \
	--tag "cli-$current_version" \
	--name "linux-cli-sha256-$sha" \
	--file LINUX_CLI_SHA256

echo "Verify this sha is a part of the release on github: $sha"
