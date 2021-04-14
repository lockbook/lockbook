#!/bin/bash

set -e

if [ -z "$LOCKBOOK_CLI_PPA_LOCATION" ]
then
	echo "No LOCKBOOK_CLI_PPA_LOCATION, we need this to find the package to update"
	exit 69
fi

if [ -z "$LOCKBOOK_DESKTOP_PPA_LOCATION" ]
then
	echo "No LOCKBOOK_DESKTOP_PPA_LOCATION, we need this to find the package to update"
	exit 69
fi

if [ -z "$DEBFULLNAME" ]
then
	echo "No DEBFULLNAME, we need this to show who bumped the version"
	exit 69
fi

if [ -z "$DEBEMAIL" ]
then
	echo "No DEBEMAIL, we need this to show who bumped the version"
	exit 69
fi

current_branch=$(git rev-parse --abbrev-ref HEAD)
current_hash=$(git rev-parse --short HEAD)

if [ $current_branch != "master" ]
then
	echo "You are not on master, don't bump version from a non-master cli. If you do Parth will be upset."
	exit 69
fi

if ! command -v dch &> /dev/null
then
	echo "You do not have the util dch, this is needed to bump the version automatically"
	exit 69
fi

cd ../clients/cli
current_version_cli=$(grep '^version =' Cargo.toml|head -n1|cut -d\" -f2|cut -d\- -f1)

cd ../linux
current_version_linux=$(grep '^version =' Cargo.toml|head -n1|cut -d\" -f2|cut -d\- -f1)

cd $LOCKBOOK_CLI_PPA_LOCATION
dch -v $current_version_cli "Automatic version bump."

git add -A
git commit -m "Manual version bump by $(git config user.name) from $current_hash"
git push

cd $LOCKBOOK_DESKTOP_PPA_LOCATION
dch -v $current_version_linux "Automatic version bump."

git add -A
git commit -m "Manual version bump by $(git config user.name) from $current_hash"
git push

echo "Debian Linux and CLI versions bumped."
