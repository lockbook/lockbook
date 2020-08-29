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

.PHONY: apple
apple:
	@{ command -v cargo || { echo "Y'ain't got cargo"; exit 1; } }

	@echo "Creating header"
	@cbindgen src/c_interface.rs -l c > lockbook_core.h

	@echo "Building fat library"
	@cargo lipo --release

	@echo "Purge/create library folders"
	@rm -rf ${inc_apple} ${libs_apple}
	@mkdir ${inc_apple}
	@mkdir ${libs_apple}

	@echo "Copying headers"
	@cp lockbook_core.h ${inc_apple}

	@echo "Copying fat library"
	@cp target/universal/release/liblockbook_core.a ${libs_apple}

.PHONY: swift
swift:
	cbindgen src/c_interface.rs -l c > lockbook_core.h
	mkdir -p ${swift_inc}
	cp lockbook_core.h ${swift_inc}
	cargo lipo --release
	mkdir -p ${swift_lib}
	cp target/universal/release/liblockbook_core.a ${swift_lib}

MIN_VER = 22
jniLibs = '../clients/android/core/src/main/jniLibs'
libName = 'liblockbook_core.so'
macosLibName = 'liblockbook_core.dylib'
windowsLibName = 'liblockbook_core.dll'

# Apple Library Location
inc_apple := ../clients/apple/include/
libs_apple := ../clients/apple/libs/

# Swift Library Location
swift_inc := ../clients/swift/CLockbookCore/Sources/CLockbookCore/include/
swift_lib := ../clients/swift/CLockbookCore/Sources/CLockbookCore/lib/
