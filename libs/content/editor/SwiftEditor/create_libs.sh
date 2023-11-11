#!/bin/bash
set -ae
rm -rf Libs/*
cd ../egui_editor
mkdir -p include
cbindgen -l c > include/egui_editor.h
pwd
cargo build --lib --release \
  --target=aarch64-apple-darwin \
  --target=x86_64-apple-darwin \
  --target=aarch64-apple-ios \
  --target=aarch64-apple-ios-sim

cd ../../../../
mkdir -p libs/content/editor/SwiftEditor/Libs
lipo -create target/x86_64-apple-darwin/release/libegui_editor.a target/aarch64-apple-darwin/release/libegui_editor.a -output libs/content/editor/SwiftEditor/Libs/libegui_editor.a
cd libs/content/editor/SwiftEditor/Libs
xcodebuild -create-xcframework \
  -library libegui_editor.a -headers ../../egui_editor/include \
  -library ../../../../../target/aarch64-apple-ios/release/libegui_editor.a -headers ../../egui_editor/include \
  -library ../../../../../target/aarch64-apple-ios-sim/release/libegui_editor.a -headers ../../egui_editor/include \
  -output egui_editor.xcframework

rm -rf libegui_editor.a
