#!/bin/bash
set -ae
rm -rf Libs/*
cd ..
mkdir -p include
cbindgen -l c > include/workspace.h
pwd

cargo build --lib --release --target=aarch64-apple-darwin
cargo build --lib --release --target=x86_64-apple-darwin
cargo build --lib --release --target=aarch64-apple-ios
cargo build --lib --release --target=aarch64-apple-ios-sim

cd ../../../
mkdir -p libs/content/workspace-ffi/SwiftWorkspace/Libs
lipo -create target/x86_64-apple-darwin/release/libworkspace.a target/aarch64-apple-darwin/release/libworkspace.a -output libs/content/workspace-ffi/SwiftWorkspace/Libs/libworkspace.a
cd libs/content/workspace-ffi/SwiftWorkspace/Libs
xcodebuild -create-xcframework \
  -library libworkspace.a -headers ../../include \
  -library ../../../../../target/aarch64-apple-ios/release/libworkspace.a -headers ../../include \
  -library ../../../../../target/aarch64-apple-ios-sim/release/libworkspace.a -headers ../../include \
  -output workspace.xcframework
