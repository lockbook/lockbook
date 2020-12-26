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

if [ -z "$ANDROID_SDK_HOME" ]
then
    echo "No ANDROID_SDK_HOME means you can't sign the lockbook apk, set it to where your sdk is."
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

if ! command -v jarsigner &> /dev/null
then
	echo "You do not have the util jarsigner, install a JDK (Java Development Kit)"
	exit 69
fi

if [ $current_branch != "master" ]
then
	echo "Do not release non-master code."
	exit 69
fi

current_branch=$(git rev-parse --abbrev-ref HEAD)
current_hash=$(git rev-parse --short HEAD)


echo "Performing clean build"
cd ../core
touch src/lib.rs
# TODO @parth do this everywhere
make android

echo "Creating apk"
cd ../clients/android/
./gradlew clean assembleRelease
jarsigner -keystore $ANDROID_RELEASE_KEY -storepass $ANDROID_RELEASE_KEY_PASSWORD app/build/outputs/apk/release/app-release-unsigned.apk android-lockbook
cd app/build/outputs/apk/release/
$ANDROID_SDK_HOME/build-tools/29.0.3/zipalign -v 4 app-release-unsigned.apk lockbook-android.apk

echo "Extracting information from release apk."
current_version=$($ANDROID_SDK_HOME/build-tools/29.0.3/aapt2 dump badging lockbook-android.apk | grep "VersionName" | sed -e "s/.*versionName='//" -e "s/' .*//")
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
