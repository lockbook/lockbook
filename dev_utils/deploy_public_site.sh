#!/bin/sh

set -e 
current_branch=$(git rev-parse --abbrev-ref HEAD)

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
git checkout gh-pages
rm -rf *
mv $temp_directory/* .
