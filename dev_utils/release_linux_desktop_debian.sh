#!/bin/bash

set -ae

if [ -z "$LOCKBOOK_DESKTOP_PPA_LOCATION" ]
then
	echo "No LOCKBOOK_DESKTOP_PPA_LOCATION, we need this to find the package"
	exit 69
fi

if [ -z "$GITHUB_TOKEN" ]
then
	echo "No GITHUB_TOKEN, you won't be able to upload to github without this"
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

if ! command -v debuild &> /dev/null
then
	echo "You do not have the util debuild, this is used to compile the package"
	exit 69
fi

if ! command -v dh &> /dev/null
then
	echo "You do not have the util debhelper, this is used to compile the package"
	exit 69
fi

if ! command -v equivs-build &> /dev/null
then
	echo "You do not have the util equivs, this is used to build the source package"
	exit 69
fi


cd $LOCKBOOK_DESKTOP_PPA_LOCATION

current_version=$(dpkg-parsechangelog --show-field Version)

echo "Setting up clean environment"
debuild -- clean

echo "Compiling package"
debuild 

cd ..

sha_description=$(shasum -a 256 lockbook-desktop_${current_version}_amd64.deb)
sha=$(echo $sha_description | cut -d ' ' -f 1)

echo "Releasing..."
github-release release \
	--user lockbook \
	--repo lockbook \
	--tag "debian-desktop-$current_version" \
	--name "Lockbook Desktop Debian" \
	--description "A debian package that installs lockbook desktop." \
	--pre-release || echo "Failed to create release, perhaps because one exists, attempting upload"

github-release upload \
	--user lockbook \
	--repo lockbook \
	--tag "debian-desktop-$current_version" \
	--name "lockbook-desktop_${current_version}_amd64.deb" \
	--file "lockbook-desktop_${current_version}_amd64.deb"

echo $sha_description >> DEBIAN_DESKTOP_SHA256

github-release upload \
	--user lockbook \
	--repo lockbook \
	--tag "debian-desktop-$current_version" \
	--name "debian-desktop-sha256-$sha" \
	--file DEBIAN_DESKTOP_SHA256

echo "Cleaning up"
rm -f "lockbook-desktop_${current_version}_amd64.build" \
	"lockbook-desktop_${current_version}_amd64.buildinfo" \
	"lockbook-desktop_${current_version}_amd64.changes" \
	"lockbook-desktop_${current_version}_amd64.deb" \
	"lockbook-desktop_${current_version}.dsc" \
	"lockbook-desktop_${current_version}.tar.gz" \
	DEBIAN_DESKTOP_SHA256 

echo "Verify this sha is a part of the release on github: $sha"
