.PHONY: android
android:
	cargo ndk --target aarch64-linux-android --android-platform $(MIN_VER) -- build --release
	cargo ndk --target armv7-linux-androideabi --android-platform $(MIN_VER) -- build --release
	cargo ndk --target i686-linux-android --android-platform $(MIN_VER) -- build --release
	cargo ndk --target x86_64-linux-android --android-platform $(MIN_VER) -- build --release

	@echo "Cleaning up old .so's if they exist"
	@rm -rf $(jniLibs)/*

	@echo "Moving new .so's over"
	@mkdir -p $(jniLibs)/arm64-v8a
	@mkdir $(jniLibs)/armeabi-v7a
	@mkdir $(jniLibs)/x86
	@mkdir $(jniLibs)/x86_64
	
	@cp target/aarch64-linux-android/release/$(libName) $(jniLibs)/arm64-v8a/$(libName)
	@cp target/armv7-linux-androideabi/release/$(libName) $(jniLibs)/armeabi-v7a/$(libName)
	@cp target/i686-linux-android/release/$(libName) $(jniLibs)/x86/$(libName)
	@cp target/x86_64-linux-android/release/$(libName) $(jniLibs)/x86_64/$(libName)

.PHONY: macos_jni
macos_jni:
	@touch src/lib.rs
	@cargo build --lib --release

	@echo "Cleaning up old .dylib if they exist"
	@rm -rf $(jniLibs)/desktop

	@echo "Moving new .dylib over"
	@mkdir -p $(jniLibs)/desktop

	@cp target/release/$(macosLibName) $(jniLibs)/desktop/$(macosLibName)

.PHONY: windows_jni
windows_jni:
	@touch src/lib.rs
	@cargo build --lib --release

	@echo "Cleaning up old .dll if they exist"
	@rm -rf $(jniLibs)/desktop

	@echo "Moving new .dll over"
	@mkdir -p $(jniLibs)/desktop

	@cp target/release/$(windowsLibName) $(jniLibs)/desktop/$(windowsLibName)

.PHONY: linux_jni
linux_jni:
	@touch src/lib.rs
	@cargo build --lib --release

	@echo "Cleaning up old .so if they exist"
	@rm -rf $(jniLibs)/desktop

	@echo "Moving new .so over"
	@mkdir -p $(jniLibs)/desktop

	@cp target/release/$(libName) $(jniLibs)/desktop/$(libName)

.PHONY: lib_c_for_swift_ios
lib_c_for_swift_ios:
	@{ command -v cargo || { echo "Y'ain't got cargo"; exit 1; } }
	@echo "Creating header"
	@rm ${swift_inc}lockbook_core.h || true
	cbindgen src/c_interface.rs -l c > lockbook_core.h
	@mkdir -p ${swift_inc}
	cp lockbook_core.h ${swift_inc}
	@echo "Building fat library"
	@rm ${swift_lib_ios}liblockbook_core.a || true
	cargo lipo --release
	@mkdir -p ${swift_lib_ios}
	cp target/universal/release/liblockbook_core.a ${swift_lib_ios}

.PHONY: lib_c_for_swift
lib_c_for_swift:
	@{ command -v cargo || { echo "Y'ain't got cargo"; exit 1; } }
	@echo "Creating header"
	@rm ${swift_inc}lockbook_core.h || true
	cbindgen src/c_interface.rs -l c > lockbook_core.h
	@mkdir -p ${swift_inc}
	cp lockbook_core.h ${swift_inc}
	@echo "Building fat library"
	@rm ${swift_lib}liblockbook_core.a || true
	cargo build --release
	@mkdir -p ${swift_lib}
	cp target/release/liblockbook_core.a ${swift_lib}

.PHONY: swift_libs
swift_libs: lib_c_for_swift lib_c_for_swift_ios

MIN_VER = 22
jniLibs = '../clients/android/core/src/main/jniLibs'
libName = 'liblockbook_core.so'
macosLibName = 'liblockbook_core.dylib'
windowsLibName = 'liblockbook_core.dll'

# Swift Library Location
swift_inc := ../clients/apple/CLockbookCore/Sources/CLockbookCore/include/
swift_lib := ../clients/apple/CLockbookCore/Sources/CLockbookCore/lib/
swift_lib_ios := ../clients/apple/CLockbookCore/Sources/CLockbookCore/lib_ios/
