#!/bin/sh

set -ea

if [ -z "$1" ];
then
echo "Apple autorun: utility script to build and launch apps on macOS or iOS without xcode \n\n<target-platform> required (iOS|macOS)\n<build-deps> optional. the deps you'd like to build and can be (all|workspace|lb)\n"
fi

projRoot=`git rev-parse --show-toplevel`



build_workspace() {
	echo "building workspace"
	cd "$projRoot"/libs/content/workspace-ffi/SwiftWorkspace
	./create_libs.sh
}

build_lb_rs() {
	echo "building lb-rs"
	cd "$projRoot"/libs/lb/lb_external_interface
	make swift_libs
}

# build workspace
case "$2" in
	"workspace")
	build_workspace
	;;
  "lb-rs")
	build_lb_rs
	;;
	"all")
	build_workspace
	build_lb_rs
	;;
	*)
	;;
	esac





# build and run app
cd "$projRoot"/clients/apple

case "$1" in
		"iOS")
# get the device id
		deviceName=$(xcrun devicectl list devices --hide-default-columns --columns "Name" --hide-headers | sed -n '2p' | xargs)
		           echo running on $deviceName

		if [ -z "$deviceName" ];
		then
		echo "No target device was found, make sure your ipad/iphone are on the same network"
		exit 1
		fi
		xcodebuild -workspace ./lockbook.xcworkspace -scheme "Lockbook (iOS)" -sdk iphoneos -configuration Debug -archivePath ./build/Lockbook-iOS.xcarchive archive
		echo $deviceName
		appBundlePath=$(xcrun devicectl device install app --device "$deviceName" ./build/Lockbook-iOS.xcarchive/Products/Applications/Lockbook.app/ | grep "installationURL:" | sed 's/.*installationURL: //')
		              xcrun devicectl device process launch --console --device "$deviceName" $appBundlePath

		              ;;
		"macOS")
		xcodebuild -workspace ./lockbook.xcworkspace -scheme "Lockbook (macOS)" -sdk macosx -configuration Debug -archivePath ./build/Lockbook-macOS.xcarchive archive
		./build/Lockbook-macOS.xcarchive/Products/Applications/Lockbook.app/Contents/MacOS/Lockbook
		;;
		esac
