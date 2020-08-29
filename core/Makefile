.PHONY: android
android:
	cargo ndk --target aarch64-linux-android --android-platform $(MIN_VER) -- build --release
	cargo ndk --target armv7-linux-androideabi --android-platform $(MIN_VER) -- build --release
	cargo ndk --target i686-linux-android --android-platform $(MIN_VER) -- build --release
	cargo ndk --target x86_64-linux-android --android-platform $(MIN_VER) -- build --release

	@echo "Cleaning up old .so's if they exist"
	@rm -rf $(jniLibs)

	@echo "Moving new .so's over"
	@mkdir $(jniLibs)
	@mkdir $(jniLibs)/arm64-v8a
	@mkdir $(jniLibs)/armeabi-v7a
	@mkdir $(jniLibs)/x86
	@mkdir $(jniLibs)/x86_64
	
	@cp target/aarch64-linux-android/release/$(libName) $(jniLibs)/arm64-v8a/$(libName)
	@cp target/armv7-linux-androideabi/release/$(libName) $(jniLibs)/armeabi-v7a/$(libName)
	@cp target/i686-linux-android/release/$(libName) $(jniLibs)/x86/$(libName)
	@cp target/x86_64-linux-android/release/$(libName) $(jniLibs)/x86_64/$(libName)

.PHONY: swift
swift:
	@{ command -v cargo || { echo "Y'ain't got cargo"; exit 1; } }
	@echo "Creating header"
	@rm ${swift_inc}lockbook_core.h || true
	cbindgen src/c_interface.rs -l c > lockbook_core.h
	@mkdir -p ${swift_inc}
	cp lockbook_core.h ${swift_inc}
	@echo "Building fat library"
	@rm ${swift_lib}liblockbook_core.a || true
	cargo lipo --release
	@mkdir -p ${swift_lib}
	cp target/universal/release/liblockbook_core.a ${swift_lib}

MIN_VER = 22
jniLibs = '../clients/android/core/src/main/jniLibs'
libName = 'liblockbook_core.so'

# Swift Library Location
swift_inc := ../clients/apple/CLockbookCore/Sources/CLockbookCore/include/
swift_lib := ../clients/apple/CLockbookCore/Sources/CLockbookCore/lib/
