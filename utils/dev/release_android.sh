#!/bin/sh
# to generate key run: 
# keytool -genkey -v -keystore android-lockbook-release-key.keystore -alias android-lockbook -keyalg RSA -keysize 2048 -validity 10000

set -ae
API_URL="http://api.lockbook.app:8000"

if [ -z "$GITHUB_TOKEN" ]
then
	echo "No GITHUB_TOKEN you won't be able to upload to github without this"
	exit 69
fi

if [ -z "$ANDROID_NDK_HOME" ]
then
    echo "No ANDROID_NDK_HOME means you can't build for android architectures."
    exit 69
fi

if [ -z "$ANDROID_RELEASE_KEY" ]
then
    echo "No ANDROID_RELEASE_KEY means you can't sign the app yourself."
    exit 69
fi

if [ -z "$ANDROID_RELEASE_KEY_PASSWORD" ]
then
    echo "No ANDROID_RELEASE_KEY_PASSWORD means you can't sign the app yourself."
    exit 69
fi

if ! command -v github-release &> /dev/null
then
	echo "You do not have the util github-release, checkout https://github.com/github-release/github-release"
	exit 69
fi

if ! command -v apksigner &> /dev/null
then
	echo "You do not have the util apksigner, install the android SDK (Software Development Kit)"
	exit 69
fi

if ! command -v aapt2 &> /dev/null
then
	echo "You do not have the util aapt2, install the android SDK (Software Development Kit)"
	exit 69
fi

if ! command -v zipalign &> /dev/null
then
	echo "You do not have the util zipalign, install the android SDK (Software Development Kit)"
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
cd ../../core
touch src/lib.rs
# TODO @parth do this everywhere
make android

echo "Creating apk"
cd ../clients/android/
./gradlew clean assembleRelease
cd app/build/outputs/apk/release/
apksigner sign --ks $ANDROID_RELEASE_KEY --ks-pass file:$ANDROID_RELEASE_KEY_PASSWORD --out lockbook-android.apk --v1-signing-enabled true --v2-signing-enabled true --v3-signing-enabled true --v4-signing-enabled true app-release-unsigned.apk 

echo "Extracting information from release apk."
current_version=$(aapt2 dump badging lockbook-android.apk | grep "VersionName" | sed -e "s/.*versionName='//" -e "s/' .*//")
sha_description=$(shasum -a 256 lockbook-android.apk)
sha=$(echo $sha_description | cut -d ' ' -f 1)

echo "Releasing..."
github-release release \
	--user lockbook \
	--repo lockbook \
	--tag "android-$current_version" \
	--name "Lockbook Android" \
	--description "Android version of lockbook." \
	--pre-release

github-release upload \
	--user lockbook \
	--repo lockbook \
	--tag "android-$current_version" \
	--name "lockbook-android.apk" \
	--file lockbook-android.apk

echo $sha_description >> ANDROID-SHA256

github-release upload \
	--user lockbook \
	--repo lockbook \
	--tag "android-$current_version" \
	--name "android-sha256-$sha" \
	--file ANDROID-SHA256

echo "Cleaning up old apks"
cd ../../../../../
./gradlew clean

echo "Verify this sha is part of the realse on github: $sha"
