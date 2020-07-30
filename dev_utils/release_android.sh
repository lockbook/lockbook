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

if [! -z "ANDROID_NDK_HOME"]
then
    echo "No ANDROID_NDK_HOME means you can't build for android architectures."
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
cd core
make android

echo "Creating apk"
cd ../clients/android/
./gradlew assembleRelease
jarsigner -keystore my-release-key.jks -storepass lockbook-android app/build/outputs/apk/release/app-release-unsigned.apk lockbook-android-release
cd app/build/outputs/apk/release/
~/Library/Android/sdk/build-tools/29.0.3/zipalign -v 4 app-release-unsigned.apk lockbook-android.apk

echo "Extracting information from release apk."
current_version=$(~/Library/Android/sdk/build-tools/29.0.3/aapt2 dump badging lockbook-android.apk | grep "VersionName" | sed -e "s/.*versionName='//" -e "s/' .*//")
sha_description=$(shasum -a 256 lockbook-android.apk)
sha=$(echo $sha_description | cut -d ' ' -f 1)

echo "Releasing..."
github-release release \
	--user lockbook \
	--repo lockbook \
	--tag $current_version \
	--name "Lockbook Android" \
	--description "Android version of lockbook." \
	--pre-release

github-release upload \
	--user lockbook \
	--repo lockbook \
	--tag $current_version \
	--name "lockbook-android.apk" \
	--file lockbook-android.apk

echo $sha_description >> ANDROID-SHA256

github-release upload \
	--user lockbook \
	--repo lockbook \
	--tag $current_version \
	--name "android-sha256-$sha" \
	--file ANDROID-SHA256

