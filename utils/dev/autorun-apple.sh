#!/bin/sh

set -ea

projRoot=`git rev-parse --show-toplevel`

# build workspace 
cd "$projRoot"/libs/content/workspace-ffi/SwiftWorkspace 
./create_libs.sh 


# build ios app 
cd "$projRoot"/clients/apple
xcodebuild -workspace ./lockbook.xcworkspace -scheme "Lockbook (iOS)" -sdk iphoneos17.5 -configuration Debug -archivePath ./build/Lockbook-iOS.xcarchive archive 
appBundlePath=$(xcrun devicectl device install app --device "$1" ./build/Lockbook-iOS.xcarchive/Products/Applications/Lockbook.app/ | grep "installationURL:" | sed 's/.*installationURL: //')

# run the app 
xcrun devicectl device process launch --console --device "$1" $appBundlePath