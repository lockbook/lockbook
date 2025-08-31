#!/bin/bash
set -ae
cd "$(dirname "$0")"

cargo ndk --target aarch64-linux-android --target armv7-linux-androideabi --target i686-linux-android --target x86_64-linux-android --platform 22 -- build --release

cd ../../../

rm -rf clients/android/workspace/src/main/jniLibs/*

mkdir -p clients/android/workspace/src/main/jniLibs/arm64-v8a/
mkdir -p clients/android/workspace/src/main/jniLibs/armeabi-v7a/
mkdir -p clients/android/workspace/src/main/jniLibs/x86/
mkdir -p clients/android/workspace/src/main/jniLibs/x86_64/

cp target/aarch64-linux-android/release/libworkspace.so clients/android/workspace/src/main/jniLibs/arm64-v8a/
cp target/armv7-linux-androideabi/release/libworkspace.so clients/android/workspace/src/main/jniLibs/armeabi-v7a/
cp target/i686-linux-android/release/libworkspace.so clients/android/workspace/src/main/jniLibs/x86/
cp target/x86_64-linux-android/release/libworkspace.so clients/android/workspace/src/main/jniLibs/x86_64/
