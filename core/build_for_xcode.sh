#!/usr/bin/env bash

{ command -v cargo || { echo "Y'ain't got cargo"; exit 1; } }

echo "Creating header"
cbindgen src/c_interface.rs -l c > lockbook_core.h

echo "Building fat library"
cargo lipo --release

inc=../clients/apple/include/
libs=../clients/apple/libs/

echo "Purge/create library folders"
rm -rf ${inc} ${libs}
mkdir ${inc}
mkdir ${libs}

echo "Copying headers"
cp lockbook_core.h ${inc}

echo "Copying fat library"
cp target/universal/release/liblockbook_core.a ${libs}
