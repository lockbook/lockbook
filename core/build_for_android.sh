#!/usr/bin/env bash
export PKG_CONFIG_ALLOW_CROSS=1
# set the version to use the library
min_ver=22
# verify before executing this that you have the proper targets installed
cargo ndk --target aarch64-linux-android --android-platform ${min_ver} -- build --release
cargo ndk --target armv7-linux-androideabi --android-platform ${min_ver} -- build --release
cargo ndk --target i686-linux-android --android-platform ${min_ver} -- build --release
cargo ndk --target x86_64-linux-android --android-platform ${min_ver} -- build --release

# moving libraries to the android project
jniLibs=../clients/android/core/src/main/jniLibs
libName=liblockbook_core.so

rm -rf ${jniLibs}

mkdir ${jniLibs}
mkdir ${jniLibs}/arm64-v8a
mkdir ${jniLibs}/armeabi-v7a
mkdir ${jniLibs}/x86
mkdir ${jniLibs}/x86_64

cp target/aarch64-linux-android/release/${libName} ${jniLibs}/arm64-v8a/${libName}
cp target/armv7-linux-androideabi/release/${libName} ${jniLibs}/armeabi-v7a/${libName}
cp target/i686-linux-android/release/${libName} ${jniLibs}/x86/${libName}
cp target/x86_64-linux-android/release/${libName} ${jniLibs}/x86_64/${libName}
