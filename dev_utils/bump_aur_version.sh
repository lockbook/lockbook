#!/bin/sh

set -e

current_branch=$(git rev-parse --abbrev-ref HEAD)
current_hash=$(git rev-parse --short HEAD)

if [ $current_branch != "master" ]
then
	echo "You are not on master, don't bump version from a non-master cli. If you do Parth will be upset."
	exit 69
fi

cd ../clients/cli
current_version=$(grep '^version =' Cargo.toml|head -n1|cut -d\" -f2|cut -d\- -f1)

cd ../../../aur-lockbook
sed -i "s/pkgver=.*/pkgver=$current_version/g" PKGBUILD
makepkg --printsrcinfo > .SRCINFO

git add -A
git commit -m "Manual deploy by $(git config user.name) from $current_hash"
git push

echo "Aur version bumped successfully"
