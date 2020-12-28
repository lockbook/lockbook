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
tar -czf lockbook-cli-macos.tar.gz lockbook
sha_description=$(shasum -a 256 lockbook-cli-macos.tar.gz)
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
	--name "lockbook-cli-macos.tar.gz" \
	--file lockbook-cli-macos.tar.gz

echo $sha_description >> MACOS_CLI_SHA256

github-release upload \
	--user lockbook \
	--repo lockbook \
	--tag "cli-$current_version" \
	--name "macos-cli-sha256-$sha" \
	--file MACOS_CLI_SHA256

echo "Verify this sha is a part of the release on github: $sha"

cd ../../../../../homebrew-lockbook/Formula
sed -i '' 's=url.*=url "https://github.com/lockbook/lockbook/releases/download/'$current_version'/lockbook-cli-macos.tar.gz"=g' lockbook.rb
sed -i '' "s/sha256.*/sha256 \"$sha\"/g" lockbook.rb
sed -i '' "s/version.*/version \"$current_version\"/g" lockbook.rb

git add -A
git commit -m "Manual deploy by $(git config user.name) from $current_hash"
git push

echo "Complete, read the logs."
