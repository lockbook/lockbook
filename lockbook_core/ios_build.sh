#!/usr/bin/env bash
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
