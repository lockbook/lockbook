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
        shift # Remove param from processing
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

[ $BUILD_FAT == 1 ] && { command -v cargo || { echo "Y'ain't got cargo"; exit 1; } }

echo "Creating header"
cbindgen src/c_interface.rs -l c > lockbook_core.h

if [ $BUILD_FAT == 1 ]
then
  echo "Building fat library"
  cargo lipo --release
fi

if [ $DO_COPY == 1 ]
then
  inc=../clients/apple/include/
  libs=../clients/apple/libs/

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
fi
