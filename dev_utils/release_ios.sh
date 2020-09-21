#!/bin/bash
set -e

if [ "$ASCPW" ]; then
    project_root=$( cd "$(dirname "${BASH_SOURCE[0]}")"/.. ; pwd -P )
    echo "Project Dir: $project_root"
    apple="$project_root/clients/apple2020"
    echo "Apple Dir: $apple"
    core="$project_root/core"
    echo "Core Dir: $core"

    echo "Triggering Rebuild"
    cd "$core"
    touch src/lib.rs
    API_URL="http://api.lockbook.app:8000" make lib_c_for_swift_ios

    echo "Archiving Lockbook (iOS)"
    xcodebuild -workspace "$apple"/Lockbook.xcworkspace \
              -scheme Lockbook\ \(iOS\) \
              -sdk iphoneos \
              -configuration Release \
              -archivePath "$apple"/build/Lockbook-iOS.xcarchive \
              archive

    echo "Export Lockbook (iOS)"
    xcodebuild \
      -allowProvisioningUpdates \
      -archivePath "$apple"/build/Lockbook-iOS.xcarchive \
      -exportPath "$apple"/build \
      -exportOptionsPlist "$apple"/exportOptions.plist \
      -exportArchive

    echo "Uploading to AppStoreConnect"
    xcrun altool \
      --upload-app \
      -t ios \
      -f "$apple"/build/Lockbook.ipa \
      -u raayanp01@gmail.com \
      -p $ASCPW
else
    echo "Set AppStoreConnect Password (ASCPW)!"
fi
