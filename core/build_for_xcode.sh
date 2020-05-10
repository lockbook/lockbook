#!/usr/bin/env bash

BUILD_FAT=1
BUILD_CATALYST=1
DO_COPY=1

for arg in "$@"
do
    case $arg in
        -f|--fat)
        BUILD_FAT=1
        BUILD_CATALYST=0
        echo "Only building the fatty"
        shift # Remove --initialize from processing
        ;;
        -c|--catalyst)
        BUILD_FAT=0
        BUILD_CATALYST=1
        echo "Only building for catalyst"
        shift
        ;;
        -n|--nocopy)
        DO_COPY=0
        echo "Not copying to client directory"
        shift
        ;;
        *)
        OTHER_ARGUMENTS+=("$1")
        shift # Remove generic argument from processing
        ;;
    esac
done

echo "Creating header"
cbindgen src/lib.rs -l c > lockbook_core.h

if [ $BUILD_FAT == 1 ]
then
  echo "Building fat library"
  cargo lipo --release
fi

if [ $BUILD_CATALYST == 1 ]
then
echo "Building for catalyst"
xargo build --target x86_64-apple-ios-macabi --release
fi

if [ $DO_COPY == 1 ]
then
  inc=../clients/ios/include/
  libs=../clients/ios/libs/

  echo "Purge/create library folders"
  rm -rf ${inc} ${libs}
  mkdir ${inc}
  mkdir ${libs}

  echo "Copying headers"
  cp lockbook_core.h ${inc}

  if [ $BUILD_FAT == 1 ]
  then
    echo "Copying fat library"
    cp target/universal/release/liblockbook_core.a ${libs}
  fi

  if [ $BUILD_CATALYST == 1 ]
  then
    echo "Copying catalyst specific library"
    cp target/x86_64-apple-ios-macabi/release/liblockbook_core.a ${libs}liblockbook_core.mac.a
  fi
fi