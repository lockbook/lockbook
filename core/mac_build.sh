#!/usr/bin/env bash
# https://robertohuertas.com/2019/10/27/rust-for-android-ios-flutter/
# cargo install cargo-lipo cbindgen
# 64 bit targets (real device & simulator):
# rustup target add aarch64-apple-ios x86_64-apple-ios
# 32 bit targets (you probably don't need these):
# rustup target add armv7-apple-ios i386-apple-ios
# building
# if you get an error about iphoneos SDK you need to:
# sudo xcode-select --switch /Applications/Xcode.app˝˝
echo "Creating header"
cbindgen src/lib.rs -l c > lockbook_core.h

echo "Building"
# build target for mac catalyst
xargo build --target x86_64-apple-ios-macabi --release

# moving files to the ios project
inc=../clients/ios/include/
libs=../clients/ios/libs/

echo "Creating library folders"
# rm -rf ${inc} ${libs}
mkdir ${inc}
mkdir ${libs}

echo "Copying headers"
cp lockbook_core.h ${inc}
echo "Copying library"
cp target/x86_64-apple-ios-macabi/release/liblockbook_core.a ${libs}liblockbook_core.mac.a
