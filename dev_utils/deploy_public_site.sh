#!/bin/sh

set -e 
current_branch=$(git rev-parse --abbrev-ref HEAD)
current_hash=$(git rev-parse --short HEAD)

if [ $current_branch != "master" ]
then
	echo "You should not deploy non-master public site code. If you do Parth will be upset."
	exit 69
fi

temp_directory=/tmp/$(date +%s)
echo "Temporary directory is: $temp_directory"

cd ../public_site
hugo

echo "Site built successfully, moving to temporary directory"
mv public $temp_directory
cd ..

echo "Checking out gh-pages"
git checkout gh-pages

echo "Deleting old things"
rm -rf *

mv $temp_directory/* .
echo "lockbook.app" >> CNAME

echo "Deploying"
git add -A
git commit -m "Manual deploy by $(git config user.name) from $current_hash"
git push origin gh-pages

echo "Switching back to original branch: $current_branch"
git checkout $current_branch
