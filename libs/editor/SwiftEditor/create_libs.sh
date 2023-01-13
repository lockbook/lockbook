#!/bin/bash
set -ae
rm -rf Libs/*
cd ../egui_editor
mkdir -p include
cbindgen -l c > include/egui_editor.h
cargo build --lib --release --target aarch64-apple-darwin --target x86_64-apple-darwin
cd ../../../
mkdir -p libs/editor/SwiftEditor/Libs
lipo -create target/x86_64-apple-darwin/release/libegui_editor.a target/aarch64-apple-darwin/release/libegui_editor.a -output libs/editor/SwiftEditor/Libs/libegui_editor.a
cd libs/editor/SwiftEditor/Libs
xcodebuild -create-xcframework -library libegui_editor.a -headers ../../egui_editor/include -output egui_editor.xcframework
rm -rf libegui_editor.a
