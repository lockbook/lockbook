.PHONY: android
android:
	cargo ndk --target aarch64-linux-android --target armv7-linux-androideabi --target i686-linux-android --target x86_64-linux-android --platform $(MIN_VER) -- build --release

	@echo "Cleaning up old .so's if they exist"
	@rm -rf $(jniLibs)/*

	@echo "Moving new .so's over"
	@mkdir -p $(jniLibs)/arm64-v8a
	@mkdir $(jniLibs)/armeabi-v7a
	@mkdir $(jniLibs)/x86
	@mkdir $(jniLibs)/x86_64

	@cp ../../../target/aarch64-linux-android/release/$(libName) $(jniLibs)/arm64-v8a/$(libName)
	@cp ../../../target/armv7-linux-androideabi/release/$(libName) $(jniLibs)/armeabi-v7a/$(libName)
	@cp ../../../target/i686-linux-android/release/$(libName) $(jniLibs)/x86/$(libName)
	@cp ../../../target/x86_64-linux-android/release/$(libName) $(jniLibs)/x86_64/$(libName)

.PHONY: macos_jni
macos_jni:
	@touch src/lib.rs
	@cargo build --lib --release

	@echo "Cleaning up old .dylib if they exist"
	@rm -rf $(jniLibs)/desktop

	@echo "Moving new .dylib over"
	@mkdir -p $(jniLibs)/desktop

	@cp ../../../target/release/$(macosLibName) $(jniLibs)/desktop/$(macosLibName)

.PHONY: windows_jni
windows_jni:
	@touch src/lib.rs
	@cargo build --lib --release

	@echo "Cleaning up old .dll if they exist"
	@rm -rf $(jniLibs)/desktop

	@echo "Moving new .dll over"
	@mkdir -p $(jniLibs)/desktop

	@cp ../../../target/release/$(windowsLibName) $(jniLibs)/desktop/$(windowsLibName)

.PHONY: linux_jni
linux_jni:
	@touch src/lib.rs
	@cargo build --lib --release

	@echo "Cleaning up old .so if they exist"
	@rm -rf $(jniLibs)/desktop

	@echo "Moving new .so over"
	@mkdir -p $(jniLibs)/desktop

	@cp ../../../target/release/$(libName) $(jniLibs)/desktop/$(libName)

.PHONY: swift_libs
swift_libs:
	@rm ${swift_inc}lb_rs.h || true
	@rm -rf ${swift_lib} ${swift_lib_ios} ${swift_lib_ios_sim}
	rm -rf ../../../target/xcode-lipo-universal

	@mkdir -p ${swift_inc}
	@mkdir -p ${swift_lib}
	@mkdir -p ${swift_lib_ios}
	@mkdir -p ${swift_lib_ios_sim}
	@mkdir -p ../../../target/xcode-lipo-universal

	cbindgen src/c_interface.rs -l c > lb_rs.h
	cargo build --release --target=aarch64-apple-ios-sim --target=aarch64-apple-ios --target=x86_64-apple-ios --target=x86_64-apple-darwin --target=aarch64-apple-darwin

	cp lb_rs.h ${swift_inc}
	cp ../../../target/aarch64-apple-ios/release/liblb_external_interface.a ${swift_lib_ios}

	lipo -create -output ${swift_lib_ios_sim}liblb_external_interface.a ../../../target/aarch64-apple-ios-sim/release/liblb_external_interface.a ../../../target/x86_64-apple-ios/release/liblb_external_interface.a
	lipo -create -output ${swift_lib}liblb_external_interface.a ../../../target/x86_64-apple-darwin/release/liblb_external_interface.a ../../../target/aarch64-apple-darwin/release/liblb_external_interface.a

# for non lipo builds (linux)
.PHONY: lib_c_for_swift_native
lib_c_for_swift_native:
	@{ command -v cargo || { echo "Y'ain't got cargo"; exit 1; } }
	@echo "Creating header"
	@rm ${swift_inc}lb_rs.h || echo "no prior .h"
	cbindgen src/c_interface.rs -l c > lb_rs.h
	@mkdir -p ${swift_inc}
	cp lb_rs.h ${swift_inc}
	@echo "Building fat library"
	cargo build --release
	@mkdir -p ${swift_lib}
	cp ../../../target/release/liblb_external_interface.a ${swift_lib}

.PHONY: lib_c_for_windows
lib_c_for_windows:
	@{ command -v cargo || { echo "Y'ain't got cargo"; exit 1; } }
	cargo build --release
	@mkdir -p ${windows_lib}
	-cp ../../../target/release/${libName} ${windows_lib}
	-cp ../../../target/release/${macosLibName} ${windows_lib}
	-cp ../../../target/release/${windowsLibName} ${windows_lib}

MIN_VER = 22
jniLibs = '../../../clients/android/lb-rs/src/main/jniLibs'
libName = 'liblb_external_interface.so'
macosLibName = 'liblb_external_interface.dylib'
windowsLibName = 'liblb_external_interface.dll'

# Swift Library Location
swift_inc := ../../../clients/apple/CLockbookCore/Sources/CLockbookCore/include/
swift_lib := ../../../clients/apple/CLockbookCore/Sources/CLockbookCore/lib/
swift_lib_ios := ../../../clients/apple/CLockbookCore/Sources/CLockbookCore/lib_ios/
swift_lib_ios_sim := ../../../clients/apple/CLockbookCore/Sources/CLockbookCore/lib_ios_sim/

# Windows Library Location
windows_lib := ../../../clients/windows/core/
