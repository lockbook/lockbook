#!/usr/bin/env bash
# https://robertohuertas.com/2019/10/27/rust-for-android-ios-flutter/
# cargo install cargo-lipo cbindgen
# 64 bit targets (real device & simulator):
# rustup target add aarch64-apple-ios x86_64-apple-ios
# 32 bit targets (you probably don't need these):
# rustup target add armv7-apple-ios i386-apple-ios
# building
cbindgen src/lib.rs -l c > lockbook_core.h
cargo lipo --release

# moving files to the ios project
inc=../ios_client/include
libs=../ios_client/libs

# rm -rf ${inc} ${libs}

# mkdir ${inc}
# mkdir ${libs}

cp lockbook_core.h ${inc}
cp target/universal/release/liblockbook_core.a ${libs}
